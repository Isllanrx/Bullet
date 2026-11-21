use super::*;

pub(super) fn load_identity(
    state_dir: &std::path::Path,
) -> Option<bullet_core::identity::IdentityLoadout> {
    let raw = std::fs::read_to_string(state_dir.join("identity.json")).ok()?;
    let parsed: bullet_core::identity::IdentityLoadout = serde_json::from_str(&raw).ok()?;
    parsed.normalized()
}

fn save_identity(
    state_dir: &std::path::Path,
    loadout: Option<&bullet_core::identity::IdentityLoadout>,
) {
    let path = state_dir.join("identity.json");
    match loadout {
        Some(value) => {
            if let Ok(bytes) = serde_json::to_vec(value) {
                let _ = bullet_platform::fs::atomic_write(&path, &bytes, false); // ignore-ok: best-effort identity persistence
            }
        }
        None => {
            let _ = std::fs::remove_file(&path); // ignore-ok: identity file may not exist yet
        }
    }
}

impl OverlaySession {
    pub(super) fn warm_champion(&self, alias: Option<String>) {
        let Some(alias) = alias else {
            return;
        };
        let Some(game_dir) = bullet_platform::paths::normalize_game_dir(&self.mods.game_dir)
            .or_else(bullet_platform::paths::discover_game_dir)
        else {
            debug!("The game folder is not known; the champion is not warmed");
            return;
        };
        let cache_dir = self.mods.state_dir.clone();
        let turn = std::sync::Arc::clone(&self.warm_turn);
        let mine = turn.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
        drop(tokio::task::spawn_blocking(move || {
            if turn.load(std::sync::atomic::Ordering::Acquire) != mine
                || bullet_classic::generator::companion_cache_is_current(
                    &game_dir, &cache_dir, &alias,
                )
            {
                return;
            }
            if let Err(e) = bullet_classic::generator::StandardChampion::open(&game_dir, &alias)
                .map(|champion| champion.with_cache_dir(&cache_dir).companions())
            {
                debug!(error = %e, "Companion characters not indexed ahead of the build");
            }
        }));
    }

    pub(super) async fn refresh_mods(
        &mut self,
        champion_id: ChampionId,
        alias: Option<String>,
    ) -> ModsPanel {
        if alias.is_none() {
            debug!(
                champion_id,
                "No WAD alias from the client; skin mods outside a champion folder are not offered"
            );
        }
        self.mod_catalog =
            mods_store::load_mod_catalog(self.mods.roots.clone(), Some(champion_id), alias).await;

        let mut selection = self.state_rx.borrow().mods.clone();
        let dropped = selection.prune(&self.mod_catalog, Some(champion_id));
        if !dropped.is_empty() {
            info!(
                dropped = ?dropped,
                "Selected custom mods are no longer on disk; removed from the selection"
            );
            mods_store::save_selection(&self.mods.state_dir, &selection);
            set_mod_selection(&self.state_tx, selection.clone());
        }

        ModsPanel {
            available: self.mod_catalog.clone(),
            selection: selection.view(Some(champion_id)),
        }
    }

    pub(super) fn custom_skin_selected(&self, champion_id: ChampionId) -> bool {
        self.state_rx.borrow().mods.skin.contains_key(&champion_id)
    }

    pub(super) fn apply_mod_request(
        &mut self,
        request: &bullet_core::mods::ModSelectionView,
        champion: Option<ChampionId>,
    ) {
        let current = self.state_rx.borrow().mods.clone();
        let (next, rejected) = self.mod_catalog.apply_request(&current, champion, request);

        for refused in &rejected {
            warn!(mod_id = %refused.id, reason = refused.reason, "Custom mod selection refused");
        }

        if next != current {
            info!(
                champion_id = ?champion,
                skin_mod = ?champion.and_then(|c| next.skin.get(&c)),
                map = ?next.map,
                font = ?next.font,
                announcer = ?next.announcer,
                others = ?next.others,
                "Custom mod selection changed"
            );
            mods_store::save_selection(&self.mods.state_dir, &next);
            set_mod_selection(&self.state_tx, next.clone());
        }

        let custom_now = champion.is_some_and(|c| next.skin.contains_key(&c));
        let restored_here = self
            .historic_restored
            .as_ref()
            .is_some_and(|restored| Some(restored.champion_id) == champion);
        if custom_now && restored_here {
            info!(
                champion_id = ?champion,
                "A custom skin was selected; the saved skin restored for this champion is dropped"
            );
            self.historic_restored = None;
            clear_overlay_target(&self.state_tx);
            self.show_selection(None, None);
        }

        self.controller.set_mod_selection(next.view(champion));
    }

    pub(super) fn dispatch_command(
        &mut self,
        command: OverlayCommand,
        catalog_champion: Option<ChampionId>,
        catalog: Option<&Catalog>,
    ) {
        match command {
            OverlayCommand::SetMods { selection } => {
                self.apply_mod_request(&selection, catalog_champion);
            }
            OverlayCommand::SetIdentity { loadout } => {
                let clean = loadout.and_then(bullet_core::identity::IdentityLoadout::normalized);
                save_identity(&self.mods.state_dir, clean.as_ref());
                bullet_core::state::set_local_identity(&self.state_tx, clean);
                info!("Customize Identity loadout updated");
            }
            OverlayCommand::JoinLobbyParty => {
                bullet_core::state::set_party_share(&self.state_tx, true);
                drop(tokio::task::spawn_blocking(|| {
                    let _ = bullet_platform::party_dialog::dismiss_lobby_invite(); // ignore-ok: dialog may not be open
                }));
                info!("Party: joined lobby sharing from in-client button");
            }
            OverlayCommand::OpenModsFolder => self.open_mods_folder(),
            OverlayCommand::OpenPanel => {
                if let Some(links) = self.panel_links.clone() {
                    bullet_platform::panel::show_panel(links);
                }
            }
            OverlayCommand::Clear => {
                self.dismiss_historic();
                self.random_declined = catalog_champion;
                handle_command(&self.state_tx, OverlayCommand::Clear, catalog);
            }
            OverlayCommand::Random => self.roll_random(catalog),
            OverlayCommand::ChromaPreview { id } => {
                self.send_chroma_preview(id, catalog);
            }
            OverlayCommand::TogglePreset => self.toggle_preset(catalog_champion),
            OverlayCommand::SetProfile { name } => {
                if self.presets.switch_to(&name) {
                    info!(profile = %name, "Skin profile switched");
                    self.profile_changed(catalog_champion);
                }
            }
            OverlayCommand::NewProfile => {
                let locale = catalog.and_then(|c| c.locale.clone());
                self.new_profile(locale.as_deref(), catalog_champion);
            }
            OverlayCommand::DeleteProfile => {
                if let Some(name) = self.presets.delete_active() {
                    info!(profile = %name, "Skin profile deleted; the default profile is active");
                    self.profile_changed(catalog_champion);
                }
            }
            OverlayCommand::FocusChampion { id } => {
                if focus_lobby_champion(&self.state_tx, id) {
                    info!(champion_id = id, "Lobby champion shown in the overlay");
                }
            }
            OverlayCommand::ImportMod { category } => {
                let locale = catalog.and_then(|c| c.locale.clone());
                let alias = catalog.and_then(|c| c.alias.clone());
                self.start_import(category, catalog_champion, alias, locale.as_deref());
            }
            other => handle_command(&self.state_tx, other, catalog),
        }
    }

    pub(super) async fn recover_reconnect_champion(&mut self) -> Option<ChampionId> {
        let client = bullet_lcu::client::LcuClient::discover().await.ok()?;
        let live = bullet_lcu::live_selection::resolve_live_selection(&client, None)
            .await
            .ok()?;
        info!(
            champion_id = live.champion_id,
            skin_id = live.skin_id,
            "Recovered champion on reconnect screen so skin selection is available"
        );
        bullet_core::state::set_champion(&self.state_tx, live.champion_id);
        if live.skin_id != 0 {
            bullet_core::state::set_selected_skin(&self.state_tx, live.skin_id);
        }
        Some(live.champion_id)
    }

    fn party_view(&self) -> bullet_core::overlay::InClientPartyView {
        let state = self.state_rx.borrow();
        let connected = match state.party_status {
            bullet_core::party::PartyStatus::Connected { members } => members,
            _ => 0,
        };
        let joinable_friends = if !state.party_share
            && connected >= 2
            && bullet_core::party::may_ask_to_share(state.phase)
        {
            connected - 1
        } else {
            0
        };
        let peer_identities = state
            .party_peers
            .iter()
            .filter_map(|p| {
                p.identity
                    .as_ref()
                    .filter(|id| !id.is_empty())
                    .map(|id| (p.puuid.clone(), id.clone()))
            })
            .collect();
        bullet_core::overlay::InClientPartyView {
            joinable_friends,
            connected_members: if state.party_share { connected } else { 0 },
            local_identity: state.local_identity.clone(),
            peer_identities,
        }
    }

    pub(super) async fn in_client_catalog_snapshot(
        &mut self,
        wanted: bool,
        use_in_client: bool,
        phase: GamePhase,
        catalog: Option<&Catalog>,
    ) -> Option<Catalog> {
        if !use_in_client || phase == GamePhase::InProgress {
            return None;
        }
        let party = self.party_view();
        if let Some(active) = catalog.filter(|_| wanted) {
            let mut copy = active.clone();
            copy.mods.selection = self.state_rx.borrow().mods.view(Some(copy.champion_id));
            copy.party = party;
            return Some(copy);
        }
        if !self.global_mods_loaded {
            let roots = self.mods.roots.clone();
            let scanned = tokio::task::spawn_blocking(move || {
                bullet_core::mods::scan_catalog(&roots, None, &|_| true)
            })
            .await
            .unwrap_or_default();
            self.mod_catalog = scanned;
            self.global_mods_loaded = true;
        }
        let selection = self.state_rx.borrow().mods.view(None);
        Some(Catalog {
            mods: ModsPanel {
                available: self.mod_catalog.clone(),
                selection,
            },
            party,
            ..Catalog::default()
        })
    }
}
