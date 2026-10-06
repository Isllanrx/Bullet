use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Portuguese,
    Spanish,
    English,
}

impl Language {
    #[must_use]
    pub fn from_locale(locale: &str) -> Option<Self> {
        let language = locale.split(['_', '-']).next()?.to_ascii_lowercase();
        match language.as_str() {
            "pt" => Some(Self::Portuguese),
            "es" => Some(Self::Spanish),
            "en" => Some(Self::English),
            _ => None,
        }
    }

    #[must_use]
    pub fn of_windows() -> Self {
        static CACHED: OnceLock<Language> = OnceLock::new();
        *CACHED.get_or_init(|| {
            let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
            Self::from_primary_lang_id(lang_id & 0x3ff)
        })
    }

    fn from_primary_lang_id(primary: u16) -> Self {
        match primary {
            0x16 => Self::Portuguese,
            0x0a => Self::Spanish,
            _ => Self::English,
        }
    }

    #[must_use]
    pub fn for_locale(locale: Option<&str>) -> Self {
        locale
            .and_then(Self::from_locale)
            .unwrap_or_else(Self::of_windows)
    }

    #[must_use]
    pub fn text(self) -> &'static Text {
        match self {
            Self::Portuguese => &PORTUGUESE,
            Self::Spanish => &SPANISH,
            Self::English => &ENGLISH,
        }
    }
}

static ACTIVE_LANGUAGE: std::sync::RwLock<Option<Language>> = std::sync::RwLock::new(None);

pub fn set_active_locale(locale: &str) {
    if let Some(lang) = Language::from_locale(locale) {
        if let Ok(mut lock) = ACTIVE_LANGUAGE.write() {
            *lock = Some(lang);
        }
    }
}

pub fn reset_active_language() {
    if let Ok(mut lock) = ACTIVE_LANGUAGE.write() {
        *lock = None;
    }
}

#[must_use]
pub fn active_language() -> Language {
    if let Ok(lock) = ACTIVE_LANGUAGE.read() {
        if let Some(lang) = *lock {
            return lang;
        }
    }
    Language::of_windows()
}

#[must_use]
pub fn text() -> &'static Text {
    active_language().text()
}

#[derive(Debug)]
pub struct Text {
    pub status_tools_missing: &'static str,
    pub status_waiting_league: &'static str,
    pub status_connected: &'static str,
    pub status_lobby: &'static str,
    pub status_matchmaking: &'static str,
    pub status_ready_check: &'static str,
    pub status_champ_select: &'static str,
    pub status_finalization: &'static str,
    pub status_injecting: &'static str,
    pub status_in_game: &'static str,
    pub status_in_game_confirmed: &'static str,
    pub status_in_game_unconfirmed: &'static str,
    pub status_in_game_failed: &'static str,
    pub status_reconnecting: &'static str,

    pub party_off: &'static str,
    pub party_unavailable: &'static str,
    pub party_connecting: &'static str,
    pub party_in_room: &'static str,
    pub party_reconnecting: &'static str,
    pub party_created_connecting: &'static str,
    pub party_created_in_room: &'static str,

    pub menu_party_create: &'static str,
    pub menu_party_join: &'static str,
    pub menu_party_leave: &'static str,
    pub menu_group_party: &'static str,
    pub menu_group_folders: &'static str,
    pub menu_open_mods: &'static str,
    pub menu_open_logs: &'static str,
    pub menu_open_tools: &'static str,
    pub menu_about: &'static str,
    pub menu_autostart: &'static str,
    pub menu_auto_accept: &'static str,
    pub menu_quit: &'static str,
    pub menu_open_panel: &'static str,
    pub menu_random_skin: &'static str,
    pub panel_section_options: &'static str,
    pub panel_section_diagnostics: &'static str,
    pub panel_random_skin_hint: &'static str,
    pub check_injector: &'static str,
    pub check_game: &'static str,
    pub check_client: &'static str,
    pub check_dll: &'static str,
    pub check_privileges: &'static str,
    pub detail_ok: &'static str,
    pub detail_injector_missing: &'static str,
    pub detail_game_missing: &'static str,
    pub detail_client_connected: &'static str,
    pub detail_client_waiting: &'static str,
    pub detail_dll_days_left: &'static str,
    pub detail_dll_refused: &'static str,
    pub detail_dll_unknown: &'static str,
    pub detail_elevated: &'static str,
    pub detail_not_elevated: &'static str,
    pub update_available_title: &'static str,
    pub update_available_body: &'static str,
    pub panel_update_line: &'static str,
    pub panel_update_download: &'static str,
    pub check_ltk: &'static str,
    pub detail_ltk_audited: &'static str,
    pub detail_ltk_unchecked: &'static str,
    pub detail_ltk_new: &'static str,
    pub panel_ltk_missing_line: &'static str,
    pub panel_ltk_new_line: &'static str,
    pub panel_ltk_download: &'static str,
    pub ltk_new_title: &'static str,
    pub ltk_new_body: &'static str,
    pub ltk_version_unknown: &'static str,
    pub panel_mark_problem: &'static str,
    pub panel_mark_problem_hint: &'static str,
    pub panel_export_diagnostics: &'static str,

    pub missing_tools_title: &'static str,
    pub missing_tools_body: &'static str,
    pub broken_tools_title: &'static str,
    pub broken_tools_body: &'static str,

    pub already_running_title: &'static str,
    pub already_running_body: &'static str,

    pub party_unavailable_title: &'static str,
    pub party_unavailable_body: &'static str,
    pub party_created_title: &'static str,
    pub party_created_body: &'static str,
    pub party_copy_failed_body: &'static str,
    pub party_join_title: &'static str,
    pub party_join_empty_clipboard: &'static str,
    pub party_join_clipboard_error: &'static str,
    pub party_joining: &'static str,
    pub party_invalid_code: &'static str,

    pub party_dialog_create_title: &'static str,
    pub party_dialog_create_desc: &'static str,
    pub party_dialog_join_title: &'static str,
    pub party_dialog_join_desc: &'static str,
    pub party_dialog_label_code: &'static str,
    pub party_dialog_placeholder: &'static str,
    pub party_dialog_btn_copy: &'static str,
    pub party_dialog_btn_paste: &'static str,
    pub party_dialog_btn_ok: &'static str,
    pub party_dialog_btn_join: &'static str,
    pub party_dialog_btn_cancel: &'static str,
    pub party_dialog_copied: &'static str,
    pub party_dialog_error_empty: &'static str,

    pub import_title: &'static str,
    pub import_refused: &'static str,
    pub import_unsupported_extension: &'static str,
    pub import_not_a_mod: &'static str,
    pub import_no_manifest: &'static str,
    pub import_no_content: &'static str,
    pub import_no_champion: &'static str,
    pub import_io_error: &'static str,

    pub html_lang: &'static str,
    pub welcome_active: &'static str,
    pub welcome_background: &'static str,
    pub welcome_author: &'static str,
    pub welcome_tray_hint: &'static str,
    pub welcome_dismiss: &'static str,

    pub welcome_quote: &'static str,
    pub party_room_full: &'static str,

    pub about_title: &'static str,
    pub about_educational: &'static str,
    pub about_quote: &'static str,
    pub about_dismiss: &'static str,
}

#[must_use]
pub fn fill(template: &str, key: &str, value: &str) -> String {
    template.replace(&format!("{{{key}}}"), value)
}

static PORTUGUESE: Text = Text {
    status_tools_missing: "Ferramentas ausentes (injeção desativada)",
    status_waiting_league: "Aguardando o League",
    status_connected: "Conectado ao League",
    status_lobby: "No lobby",
    status_matchmaking: "Buscando partida",
    status_ready_check: "Partida encontrada",
    status_champ_select: "Seleção de campeões",
    status_finalization: "Finalizando a seleção",
    status_injecting: "Injetando a skin…",
    status_in_game: "Em jogo",
    status_in_game_confirmed: "Em jogo — skin ativa",
    status_in_game_unconfirmed: "Em jogo — skin NÃO confirmada",
    status_in_game_failed: "Em jogo — falha na injeção",
    status_reconnecting: "Reconectando",

    party_off: "Party: desligado",
    party_unavailable: "Party: indisponível (relay não configurado)",
    party_connecting: "Party: conectando…",
    party_in_room: "Party: na sala ({n} no total)",
    party_reconnecting: "Party: reconectando…",
    party_created_connecting: "Party criada: conectando…",
    party_created_in_room: "Party criada: na sala ({n} no total)",

    menu_party_create: "Criar sala de party...",
    menu_party_join: "Entrar na sala de party...",
    menu_party_leave: "Sair da party",
    menu_group_party: "Grupo (Party)",
    menu_group_folders: "Pastas",
    menu_open_mods: "Abrir pasta de mods",
    menu_open_logs: "Abrir pasta de logs",
    menu_open_tools: "Abrir pasta de ferramentas",
    menu_about: "Sobre o Bullet...",
    menu_autostart: "Iniciar com o Windows",
    menu_auto_accept: "Aceitar partida automaticamente",
    menu_quit: "Sair do Bullet",
    menu_open_panel: "Abrir o Bullet",
    menu_random_skin: "Sortear skin se nenhuma for escolhida",
    panel_section_options: "Opções",
    panel_section_diagnostics: "Diagnóstico",
    panel_random_skin_hint: "Quando o campeão trava sem skin escolhida no Bullet, uma skin é sorteada para a partida não começar sem skin.",
    check_injector: "Injetor (pasta tools)",
    check_game: "Jogo instalado",
    check_client: "Cliente do League",
    check_dll: "Validade da DLL do injetor",
    check_privileges: "Privilégios do Bullet",
    detail_ok: "OK",
    detail_injector_missing: "ltk_patcher_host.exe ou ltk_patcher_dll.dll ausente",
    detail_game_missing: "pasta do jogo não encontrada",
    detail_client_connected: "conectado",
    detail_client_waiting: "aguardando o cliente abrir",
    detail_dll_days_left: "aceita o patch atual; recusa builds do jogo feitos daqui a {n} dia(s) ou mais",
    detail_dll_refused: "o patch instalado é mais novo do que a DLL aceita: nenhuma skin carrega até sair uma DLL nova",
    detail_dll_unknown: "build do jogo não lido",
    detail_elevated: "executando como administrador",
    detail_not_elevated: "executando sem elevação",
    update_available_title: "Bullet {version} disponível",
    update_available_body: "Clique aqui para abrir a página de download. Nada é baixado nem instalado sem você.",
    panel_update_line: "Nova versão {version} disponível (você usa a {current}).",
    panel_update_download: "Abrir página de download",
    check_ltk: "Injetor LTK",
    detail_ltk_audited: "build auditada; a versão mais nova do LTK Manager com esses arquivos é a {version}",
    detail_ltk_unchecked: "build auditada; versão do LTK Manager ainda não consultada",
    detail_ltk_new: "o LTK Manager {latest} trouxe um injetor novo; o Bullet passa a aceitá-lo depois de auditá-lo em uma atualização",
    panel_ltk_missing_line: "Baixe o LTK Manager {version} e copie ltk_patcher_host.exe e ltk_patcher_dll.dll para a pasta tools.",
    panel_ltk_new_line: "O LTK Manager {latest} trouxe um injetor novo. Mantenha os arquivos atuais: uma atualização do Bullet vai aceitá-lo e indicar a versão para baixar.",
    panel_ltk_download: "Abrir LTK Manager {version}",
    ltk_new_title: "LTK Manager {version} trouxe um injetor novo",
    ltk_new_body: "O Bullet só aceita o injetor depois de auditá-lo. Mantenha os arquivos atuais; uma atualização do Bullet vai indicar a versão para baixar.",
    ltk_version_unknown: "indicado no passo 2 do README",
    panel_mark_problem: "Marcar problema agora",
    panel_mark_problem_hint: "Sem sair do jogo: durante a partida, Ctrl+Shift+B marca o momento em que algo aparece errado e F12 tira um print. No fim da partida o diagnóstico é salvo sozinho na pasta de logs.",
    panel_export_diagnostics: "Exportar diagnóstico",

    missing_tools_title: "Bullet — Injetor Necessário",
    missing_tools_body: "O Bullet precisa do injetor para funcionar:\n• ltk_patcher_host.exe\n• ltk_patcher_dll.dll\n\nCopie os dois do LTK Manager {version} para a pasta 'tools' e abra o Bullet de novo.\nA pasta e a página de download do LTK Manager foram abertas para você.",
    broken_tools_title: "Bullet — Injetor Inválido",
    broken_tools_body: "Os arquivos do injetor na pasta 'tools' não são a build auditada.\n\nCopie os dois do LTK Manager {version} antes de iniciar.\nA pasta e a página de download do LTK Manager foram abertas para você.",

    already_running_title: "Bullet já está aberto",
    already_running_body: "O Bullet já está em execução em segundo plano.\n\nProcure o ícone do Bullet na \
                           bandeja do Windows.\nPara encerrá-lo, clique com o botão direito no ícone e \
                           escolha \"Sair do Bullet\".",

    party_unavailable_title: "Party indisponível",
    party_unavailable_body: "O party precisa de um relay configurado.\n\n{reason}",
    party_created_title: "Sala de party criada",
    party_created_body: "O código da sala foi copiado. Cole para seus amigos — ele vale por 1 hora.\n\n\
                         Quem tiver o código vê a skin que você escolher.",
    party_copy_failed_body: "Não foi possível copiar o código. Copie manualmente:\n\n{code}",
    party_join_title: "Entrar na party",
    party_join_empty_clipboard: "Copie o código da sala que seu amigo enviou e tente de novo.",
    party_join_clipboard_error: "A área de transferência não pôde ser lida: {error}",
    party_joining: "Entrando na sala. O status aparece no menu da bandeja.",
    party_invalid_code: "Código de party inválido: {error}",

    party_dialog_create_title: "Sala de Party Criada",
    party_dialog_create_desc: "Envie este código para seus amigos no mesmo time para verem suas skins:",
    party_dialog_join_title: "Entrar na Sala de Party",
    party_dialog_join_desc: "Insira ou cole o código da sala de party enviado pelo seu amigo:",
    party_dialog_label_code: "Código da Sala (Party)",
    party_dialog_placeholder: "Cole o código aqui (BULLET1:...)",
    party_dialog_btn_copy: "Copiar Código",
    party_dialog_btn_paste: "Colar",
    party_dialog_btn_ok: "Concluir",
    party_dialog_btn_join: "Entrar na Sala",
    party_dialog_btn_cancel: "Cancelar",
    party_dialog_copied: "Copiado! ✓",
    party_dialog_error_empty: "Por favor, insira o código da sala.",

    import_title: "Bullet — importar mod",
    import_refused: "O arquivo não foi importado:\n{reason}",
    import_unsupported_extension: "só é possível importar mods .fantome e .zip",
    import_not_a_mod: "não é um pacote de mod ({error})",
    import_no_manifest: "falta o manifesto META/info.json",
    import_no_content: "o pacote não tem conteúdo em WAD/ nem em RAW/",
    import_no_champion: "o pacote não diz de qual campeão é: escolha o campeão na seleção e importe de novo",
    import_io_error: "erro ao gravar o mod: {error}",

    html_lang: "pt-BR",
    welcome_active: "ATIVO NA BANDEJA DO SISTEMA",
    welcome_background: "O Bullet permanece minimizado em segundo plano",
    welcome_author: "Projeto desenvolvido por Isllan Toso.",
    welcome_tray_hint: "Clique no ícone da bandeja para abrir o painel: opções, party, pastas de mods e logs.",
    welcome_dismiss: "ENTENDIDO",
    welcome_quote: "“Eu sempre atiro primeiro.” — Miss Fortune",
    party_room_full: "A sala de party está cheia. Peça para alguém sair e tente entrar de novo.",

    about_title: "SOBRE O BULLET",
    about_educational: "Projeto educacional e sem fins lucrativos, para estudo de engenharia reversa, formatos de arquivo do jogo e injeção no Windows. Use por sua conta e risco: alterar o cliente viola os Termos de Serviço da Riot Games e pode levar a banimento. Bullet não é afiliado à Riot Games.",
    about_quote: "\u{201c}Eu sempre atiro primeiro.\u{201d} \u{2014} Miss Fortune",
    about_dismiss: "FECHAR",
};

static SPANISH: Text = Text {
    status_tools_missing: "Faltan las herramientas (inyección desactivada)",
    status_waiting_league: "Esperando a League",
    status_connected: "Conectado a League",
    status_lobby: "En la sala",
    status_matchmaking: "Buscando partida",
    status_ready_check: "Partida encontrada",
    status_champ_select: "Selección de campeones",
    status_finalization: "Finalizando la selección",
    status_injecting: "Inyectando el aspecto…",
    status_in_game: "En partida",
    status_in_game_confirmed: "En partida — aspecto activo",
    status_in_game_unconfirmed: "En partida — aspecto NO confirmado",
    status_in_game_failed: "En partida — fallo en la inyección",
    status_reconnecting: "Reconectando",

    party_off: "Party: desactivado",
    party_unavailable: "Party: no disponible (relay sin configurar)",
    party_connecting: "Party: conectando…",
    party_in_room: "Party: en la sala ({n} en total)",
    party_reconnecting: "Party: reconectando…",
    party_created_connecting: "Party creada: conectando…",
    party_created_in_room: "Party creada: en la sala ({n} en total)",

    menu_party_create: "Crear sala de party...",
    menu_party_join: "Unirse a la sala de party...",
    menu_party_leave: "Salir de la party",
    menu_group_party: "Grupo (Party)",
    menu_group_folders: "Carpetas",
    menu_open_mods: "Abrir carpeta de mods",
    menu_open_logs: "Abrir carpeta de registros",
    menu_open_tools: "Abrir carpeta de herramientas",
    menu_about: "Acerca de Bullet...",
    menu_autostart: "Iniciar con Windows",
    menu_auto_accept: "Aceptar partida automáticamente",
    menu_quit: "Salir de Bullet",
    menu_open_panel: "Abrir Bullet",
    menu_random_skin: "Skin aleatoria si no eliges ninguna",
    panel_section_options: "Opciones",
    panel_section_diagnostics: "Diagnóstico",
    panel_random_skin_hint: "Cuando el campeón se bloquea sin skin elegida en Bullet, se sortea una para que la partida no empiece sin skin.",
    check_injector: "Inyector (carpeta tools)",
    check_game: "Juego instalado",
    check_client: "Cliente de League",
    check_dll: "Validez de la DLL del inyector",
    check_privileges: "Privilegios de Bullet",
    detail_ok: "OK",
    detail_injector_missing: "falta ltk_patcher_host.exe o ltk_patcher_dll.dll",
    detail_game_missing: "carpeta del juego no encontrada",
    detail_client_connected: "conectado",
    detail_client_waiting: "esperando a que se abra el cliente",
    detail_dll_days_left: "acepta el parche actual; rechaza builds del juego hechas dentro de {n} día(s) o más",
    detail_dll_refused: "el parche instalado es más nuevo de lo que acepta la DLL: ninguna skin carga hasta que salga una DLL nueva",
    detail_dll_unknown: "build del juego no leída",
    detail_elevated: "ejecutando como administrador",
    detail_not_elevated: "ejecutando sin elevación",
    update_available_title: "Bullet {version} disponible",
    update_available_body: "Haz clic aquí para abrir la página de descarga. No se descarga ni se instala nada sin ti.",
    panel_update_line: "Nueva versión {version} disponible (usas la {current}).",
    panel_update_download: "Abrir página de descarga",
    check_ltk: "Inyector LTK",
    detail_ltk_audited: "build auditada; la versión más nueva de LTK Manager con estos archivos es la {version}",
    detail_ltk_unchecked: "build auditada; versión de LTK Manager aún no consultada",
    detail_ltk_new: "LTK Manager {latest} trajo un inyector nuevo; Bullet lo acepta después de auditarlo en una actualización",
    panel_ltk_missing_line: "Descarga LTK Manager {version} y copia ltk_patcher_host.exe y ltk_patcher_dll.dll a la carpeta tools.",
    panel_ltk_new_line: "LTK Manager {latest} trajo un inyector nuevo. Mantén los archivos actuales: una actualización de Bullet lo aceptará e indicará la versión que debes descargar.",
    panel_ltk_download: "Abrir LTK Manager {version}",
    ltk_new_title: "LTK Manager {version} trajo un inyector nuevo",
    ltk_new_body: "Bullet solo acepta el inyector después de auditarlo. Mantén los archivos actuales; una actualización de Bullet indicará la versión que debes descargar.",
    ltk_version_unknown: "indicado en el paso 2 del README",
    panel_mark_problem: "Marcar problema ahora",
    panel_mark_problem_hint: "Sin salir del juego: durante la partida, Ctrl+Shift+B marca el momento en que algo se ve mal y F12 toma una captura. Al terminar la partida el diagnóstico se guarda solo en la carpeta de logs.",
    panel_export_diagnostics: "Exportar diagnóstico",

    missing_tools_title: "Bullet — Inyector Requerido",
    missing_tools_body: "Bullet necesita el inyector para funcionar:\n• ltk_patcher_host.exe\n• ltk_patcher_dll.dll\n\nCópielos desde LTK Manager {version} a la carpeta 'tools' y vuelva a abrir Bullet.\nLa carpeta y la página de descarga de LTK Manager se han abierto para usted.",
    broken_tools_title: "Bullet — Inyector Inválido",
    broken_tools_body: "Los archivos del inyector en la carpeta 'tools' no son la build auditada.\n\nCopie los dos desde LTK Manager {version} antes de iniciar.\nLa carpeta y la página de descarga de LTK Manager se han abierto para usted.",

    already_running_title: "Bullet ya está abierto",
    already_running_body: "Bullet ya se está ejecutando en segundo plano.\n\nBusca el icono de Bullet en la \
                           bandeja de Windows.\nPara cerrarlo, haz clic derecho en el icono y elige \
                           \"Salir de Bullet\".",

    party_unavailable_title: "Party no disponible",
    party_unavailable_body: "La party necesita un relay configurado.\n\n{reason}",
    party_created_title: "Sala de party creada",
    party_created_body: "Se copió el código de la sala. Pégalo a tus amigos — vale por 1 hora.\n\n\
                         Quien tenga el código verá el aspecto que elijas.",
    party_copy_failed_body: "No se pudo copiar el código. Cópialo a mano:\n\n{code}",
    party_join_title: "Unirse a la party",
    party_join_empty_clipboard: "Copia el código de la sala que te envió tu amigo e inténtalo de nuevo.",
    party_join_clipboard_error: "No se pudo leer el portapapeles: {error}",
    party_joining: "Entrando en la sala. El estado aparece en el menú de la bandeja.",
    party_invalid_code: "Código de party no válido: {error}",

    party_dialog_create_title: "Sala de Party Creada",
    party_dialog_create_desc: "Envía este código a tus amigos en el mismo equipo para sincronizar aspectos:",
    party_dialog_join_title: "Unirse a la Sala de Party",
    party_dialog_join_desc: "Introduce o pega el código de sala que te envió tu amigo:",
    party_dialog_label_code: "Código de Sala (Party)",
    party_dialog_placeholder: "Pega el código aquí (BULLET1:...)",
    party_dialog_btn_copy: "Copiar Código",
    party_dialog_btn_paste: "Pegar",
    party_dialog_btn_ok: "Aceptar",
    party_dialog_btn_join: "Entrar a la Sala",
    party_dialog_btn_cancel: "Cancelar",
    party_dialog_copied: "¡Copiado! ✓",
    party_dialog_error_empty: "Por favor, introduce el código de la sala.",

    import_title: "Bullet — importar mod",
    import_refused: "El archivo no se importó:\n{reason}",
    import_unsupported_extension: "solo se pueden importar mods .fantome y .zip",
    import_not_a_mod: "no es un paquete de mod ({error})",
    import_no_manifest: "falta el manifiesto META/info.json",
    import_no_content: "el paquete no tiene contenido en WAD/ ni en RAW/",
    import_no_champion: "el paquete no indica de qué campeón es: elige el campeón en la selección e impórtalo de nuevo",
    import_io_error: "error al guardar el mod: {error}",

    html_lang: "es",
    welcome_active: "ACTIVO EN LA BANDEJA DEL SISTEMA",
    welcome_background: "Bullet sigue minimizado en segundo plano",
    welcome_author: "Proyecto desarrollado por Isllan Toso.",
    welcome_tray_hint: "Haz clic en el icono de la bandeja para abrir el panel: opciones, party, carpetas de mods y logs.",
    welcome_dismiss: "ENTENDIDO",
    welcome_quote: "",
    party_room_full: "La sala de party está llena. Pide que alguien salga e intenta entrar de nuevo.",

    about_title: "ACERCA DE BULLET",
    about_educational: "Proyecto educativo y sin fines de lucro, para el estudio de ingeniería inversa, formatos de archivo del juego e inyección en Windows. Úsalo bajo tu propio riesgo: modificar el cliente infringe los Términos de Servicio de Riot Games y puede provocar un baneo. Bullet no está afiliado a Riot Games.",
    about_quote: "\u{201c}Siempre disparo primero.\u{201d} \u{2014} Miss Fortune",
    about_dismiss: "CERRAR",
};

static ENGLISH: Text = Text {
    status_tools_missing: "Tools missing (injection disabled)",
    status_waiting_league: "Waiting for League",
    status_connected: "Connected to League",
    status_lobby: "In lobby",
    status_matchmaking: "Finding a match",
    status_ready_check: "Match found",
    status_champ_select: "Champion select",
    status_finalization: "Finalizing selection",
    status_injecting: "Injecting the skin…",
    status_in_game: "In game",
    status_in_game_confirmed: "In game — skin active",
    status_in_game_unconfirmed: "In game — skin NOT confirmed",
    status_in_game_failed: "In game — injection failed",
    status_reconnecting: "Reconnecting",

    party_off: "Party: off",
    party_unavailable: "Party: unavailable (no relay configured)",
    party_connecting: "Party: connecting…",
    party_in_room: "Party: in the room ({n} in total)",
    party_reconnecting: "Party: reconnecting…",
    party_created_connecting: "Party created: connecting…",
    party_created_in_room: "Party created: in the room ({n} in total)",

    menu_party_create: "Create party room...",
    menu_party_join: "Join party room...",
    menu_party_leave: "Leave party",
    menu_group_party: "Party",
    menu_group_folders: "Folders",
    menu_open_mods: "Open mods folder",
    menu_open_logs: "Open logs folder",
    menu_open_tools: "Open tools folder",
    menu_about: "About Bullet...",
    menu_autostart: "Start with Windows",
    menu_auto_accept: "Accept matches automatically",
    menu_quit: "Quit Bullet",
    menu_open_panel: "Open Bullet",
    menu_random_skin: "Random skin if none is chosen",
    panel_section_options: "Options",
    panel_section_diagnostics: "Diagnostics",
    panel_random_skin_hint: "When your champion locks in with no skin chosen in Bullet, one is rolled so the match never starts without a skin.",
    check_injector: "Injector (tools folder)",
    check_game: "Installed game",
    check_client: "League client",
    check_dll: "Injector DLL validity",
    check_privileges: "Bullet privileges",
    detail_ok: "OK",
    detail_injector_missing: "ltk_patcher_host.exe or ltk_patcher_dll.dll is missing",
    detail_game_missing: "game folder not found",
    detail_client_connected: "connected",
    detail_client_waiting: "waiting for the client to open",
    detail_dll_days_left: "accepts the current patch; refuses game builds made {n} day(s) from now or later",
    detail_dll_refused: "the installed patch is newer than the DLL accepts: no skin loads until a refreshed DLL ships",
    detail_dll_unknown: "game build not read",
    detail_elevated: "running as administrator",
    detail_not_elevated: "running without elevation",
    update_available_title: "Bullet {version} is available",
    update_available_body: "Click here to open the download page. Nothing is downloaded or installed without you.",
    panel_update_line: "Version {version} is available (you have {current}).",
    panel_update_download: "Open download page",
    check_ltk: "LTK injector",
    detail_ltk_audited: "audited build; the newest LTK Manager release with these files is {version}",
    detail_ltk_unchecked: "audited build; LTK Manager releases not checked yet",
    detail_ltk_new: "LTK Manager {latest} ships a new injector; Bullet accepts it once an update has audited it",
    panel_ltk_missing_line: "Download LTK Manager {version} and copy ltk_patcher_host.exe and ltk_patcher_dll.dll into the tools folder.",
    panel_ltk_new_line: "LTK Manager {latest} ships a new injector. Keep the current files: a Bullet update will accept it and name the version to download.",
    panel_ltk_download: "Open LTK Manager {version}",
    ltk_new_title: "LTK Manager {version} ships a new injector",
    ltk_new_body: "Bullet only accepts the injector after auditing it. Keep the current files; a Bullet update will name the version to download.",
    ltk_version_unknown: "named in step 2 of the README",
    panel_mark_problem: "Mark a problem now",
    panel_mark_problem_hint: "Without leaving the game: during a match, Ctrl+Shift+B marks the moment something looks wrong and F12 takes a screenshot. When the match ends the diagnostics are saved to the logs folder on their own.",
    panel_export_diagnostics: "Export diagnostics",

    missing_tools_title: "Bullet — Injector Required",
    missing_tools_body: "Bullet requires the injection backend to operate:\n• ltk_patcher_host.exe\n• ltk_patcher_dll.dll\n\nCopy both from LTK Manager {version} into the 'tools' folder and open Bullet again.\nThe folder and the LTK Manager download page have been opened for you.",
    broken_tools_title: "Bullet — Invalid Injector",
    broken_tools_body: "The injector files in the 'tools' folder are not the audited build.\n\nCopy both from LTK Manager {version} before starting.\nThe folder and the LTK Manager download page have been opened for you.",

    already_running_title: "Bullet is already open",
    already_running_body: "Bullet is already running in the background.\n\nLook for the Bullet icon in the \
                           Windows tray.\nTo close it, right-click the icon and choose \"Quit Bullet\".",

    party_unavailable_title: "Party unavailable",
    party_unavailable_body: "Party mode needs a configured relay.\n\n{reason}",
    party_created_title: "Party room created",
    party_created_body: "The room code was copied. Paste it to your friends — it is valid for 1 hour.\n\n\
                         Whoever has the code sees the skin you pick.",
    party_copy_failed_body: "The code could not be copied. Copy it by hand:\n\n{code}",
    party_join_title: "Join party",
    party_join_empty_clipboard: "Copy the room code your friend sent and try again.",
    party_join_clipboard_error: "The clipboard could not be read: {error}",
    party_joining: "Joining the room. The status shows in the tray menu.",
    party_invalid_code: "Invalid party code: {error}",

    party_dialog_create_title: "Party Room Created",
    party_dialog_create_desc: "Send this code to your teammates so they see your custom skins:",
    party_dialog_join_title: "Join Party Room",
    party_dialog_join_desc: "Enter or paste the room code sent by your friend:",
    party_dialog_label_code: "Room Code (Party)",
    party_dialog_placeholder: "Paste room code here (BULLET1:...)",
    party_dialog_btn_copy: "Copy Code",
    party_dialog_btn_paste: "Paste",
    party_dialog_btn_ok: "Done",
    party_dialog_btn_join: "Join Room",
    party_dialog_btn_cancel: "Cancel",
    party_dialog_copied: "Copied! ✓",
    party_dialog_error_empty: "Please enter the room code.",

    import_title: "Bullet — import mod",
    import_refused: "The file was not imported:\n{reason}",
    import_unsupported_extension: "only .fantome and .zip mods can be imported",
    import_not_a_mod: "not a mod package ({error})",
    import_no_manifest: "the META/info.json manifest is missing",
    import_no_content: "the package has no WAD/ or RAW/ content",
    import_no_champion: "the package does not say which champion it is for: pick the champion in champ select and import it again",
    import_io_error: "could not write the mod: {error}",

    html_lang: "en",
    welcome_active: "ACTIVE IN THE SYSTEM TRAY",
    welcome_background: "Bullet stays minimized in the background",
    welcome_author: "Project developed by Isllan Toso.",
    welcome_tray_hint: "Click the tray icon to open the control panel: options, party, and the mods and logs folders.",
    welcome_dismiss: "GOT IT",
    welcome_quote: "",
    party_room_full: "The party room is full. Ask someone to leave and try joining again.",

    about_title: "ABOUT BULLET",
    about_educational: "An educational, non-commercial project for studying reverse engineering, the game's file formats and Windows injection. Use at your own risk: modifying the client violates Riot Games' Terms of Service and may lead to a ban. Bullet is not affiliated with Riot Games.",
    about_quote: "\u{201c}I always shoot first.\u{201d} \u{2014} Miss Fortune",
    about_dismiss: "CLOSE",
};

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
