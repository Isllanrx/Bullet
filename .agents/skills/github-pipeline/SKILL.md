---
name: github-pipeline
description: Monta, audita e opera o pipeline automatizado do Bullet no GitHub — CI, segurança, auto-merge após aprovação, pre-release automática, promoção manual e notificações no Discord. Use quando pedirem "configurar CI/CD", "auto-merge", "ruleset", "proteger a main", "release automática", "promover release", "por que o PR não mergeou", ou ao alterar qualquer arquivo em .github/.
---

# Pipeline do Bullet no GitHub

Objetivo: depois que um maintainer aprova um PR, tudo segue sozinho até uma **pre-release** — e
nenhuma aprovação errada consegue colocar build ruim na mão do usuário.

## O fluxo

```
PR aberto (fork ou branch)
  ├─ ci.yml        → Format/lint/test · Release build (metadados + asInvoker) · cargo-deny
  │                  · relay typecheck + npm audit · actionlint + zizmor → "CI OK"
  └─ security.yml  → CodeQL (rust, actions, ts) · gitleaks (histórico todo) · dependency review → "Security OK"
Maintainer aprova
  └─ pr-approved.yml (token read-only) grava PR + SHA aprovado num artifact
     └─ automerge.yml (workflow_run, token do GitHub App) revalida TUDO pela API:
          PR aberto, base main, não-draft, SHA == aprovado, aprovação de quem tem write,
          ninguém pedindo mudança, sem label no-automerge, NENHUM caminho protegido
        └─ gh pr merge --auto --squash --match-head-commit <SHA>
           └─ o GitHub só mergeia quando "CI OK" + "Security OK" passam (ruleset)
Merge na main muda a versão em Cargo.toml
  └─ release.yml → gate completo sem cache · instalador (sem o LTK: a licença proíbe redistribuir)
                  · SHA256SUMS · attestation de proveniência · PRE-RELEASE
Maintainer testa em partida real
  └─ promote.yml (environment "production", revisor obrigatório)
       confere SHA256SUMS + attestation do release.yml → marca como latest
       └─ Notifica canal do Discord com link direto do instalador e SHA-256

Eventos adicionais:
  └─ star-notify.yml (watch: started) → Notifica estrela recebida no Discord
```

## Redes de segurança contra aprovação errada

| Risco | Contenção |
|---|---|
| Aprovar código quebrado | Merge só acontece com "CI OK" e "Security OK" verdes (ruleset, `strict`) |
| Push depois da aprovação | `dismiss_stale_reviews_on_push`, `require_last_push_approval` e `--match-head-commit` |
| PR de fork roubar credenciais | CI usa `pull_request` (sem secrets); nada roda código do PR com token de escrita; `persist-credentials: false` |
| Artifact forjado pelo fork | `automerge.yml` valida número/SHA por regex e rechecagem na API |
| Mudança em workflow, instalador, hashes auditados, cripto do party, toolchain, lockfile | Caminhos protegidos: auto-merge recusa; merge manual (segundo ato deliberado) + CODEOWNERS |
| Build ruim chegar ao usuário | Release automática é **pre-release**; o app não se autoatualiza; promoção exige environment com revisor |
| Binário adulterado | LTK não é redistribuído; o app confere o hash em runtime; attestation verificada antes de promover |
| Ação de terceiro comprometida | Toda `uses:` fixada por SHA de commit; Dependabot atualiza; zizmor audita |

## Configuração única (repositório)

Pré-requisito: `gh` autenticado como admin do repo (`winget install GitHub.cli`, `gh auth login`).

1. **GitHub App** (para o merge disparar a release — merges feitos com `GITHUB_TOKEN` não disparam workflows):
   - Settings → Developer settings → GitHub Apps → New. Permissões de repositório: *Contents: Read & write*,
     *Pull requests: Read & write*, *Metadata: Read*. Sem webhook. Instale só no repo `Bullet`.
   - Variável `BULLET_BOT_APP_ID` (Actions → Variables) e secret `BULLET_BOT_PRIVATE_KEY` (Actions → Secrets).
2. **Auto-merge**: Settings → General → Pull Requests → marcar *Allow auto-merge* e *Allow squash merging*;
   desmarcar merge commit e rebase.
3. **Ruleset da main**:
   ```powershell
   gh api -X POST repos/Isllanrx/Bullet/rulesets --input .github/rulesets/main.json
   ```
   Para atualizar: `gh api repos/Isllanrx/Bullet/rulesets` (pegar o id) e `-X PUT .../rulesets/<id>`.
   Os nomes "CI OK" e "Security OK" só aparecem como checks depois de rodarem uma vez.
4. **Environment `production`**: Settings → Environments → New → *Name*: `production` → *Required reviewers*: você;
   *Deployment branches*: `main`.
5. **Discord Webhooks** (Actions → Secrets):
   - `DISCORD_RELEASE_WEBHOOK`: canal de anúncios de novas releases oficiais.
   - `DISCORD_STAR_WEBHOOK`: canal de notificações de novas estrelas no repositório.
6. **Forks**: Settings → Actions → General → *Require approval for all outside collaborators*.
7. **Segurança**: Settings → Code security → ativar *Dependency graph*, *Dependabot alerts*,
   *Secret scanning* + *Push protection*, *Private vulnerability reporting*.

## Compatibilidade do Pipeline (Setembro de 2026)

- **Node.js 24 Runtime**: Desde 16 de setembro de 2026, o Node.js 20 foi descontinuado e removido dos runners do GitHub Actions. Todos os workflows utilizam `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` e Node 24 (`node-version: 24`).
- **Runners**: `windows-latest` (Windows Server 2025/2022) e `ubuntu-latest` (Ubuntu 24.04 LTS).
- **Scripts Inline (Actionlint & Shellcheck)**: Scripts Python inline em workflows devem ser encapsulados com aspas simples (`python3 -c 'import textwrap; exec(textwrap.dedent("""..."""))'`) para evitar que o linter de shell tente interpretar interpolações ou crases de markdown como código bash.

## Operação

- **Lançar versão**: PR que muda `version` em `[workspace.package]` do `Cargo.toml` → ao mergear, sai a
  pre-release `v<versão>`. Validar em partida → Actions → *Promote release* → informar a tag.
- **Impedir auto-merge de um PR**: label `no-automerge`.
- **PR em caminho protegido**: aprovar e mergear manualmente (botão *Squash and merge*).
- **PR não mergeou**: ver o log do *Auto-merge* (motivo sai como `::error::`/`::warning::`) e se
  "CI OK"/"Security OK" estão verdes.
- **Alterar um workflow**: rodar localmente antes do PR:
  ```powershell
  docker run --rm -v "${PWD}:/repo" --workdir /repo rhysd/actionlint:1.7.12 -color
  pipx run zizmor==1.30.1 --min-severity medium .github/workflows
  ```
  Toda `uses:` nova vai fixada por SHA (`git ls-remote https://github.com/<owner>/<action> refs/tags/<tag>`,
  usando o `^{}` quando a tag for anotada) com a versão em comentário.

## Invariantes (não quebrar)

- Nenhum workflow com `pull_request_target` ou que faça checkout do código do PR com token de escrita.
- `permissions` mínimas por job; `contents: write` só no job que publica ou mergeia.
- Nome dos jobs agregadores ("CI OK", "Security OK") = contexts do ruleset. Renomear um = atualizar o outro.
- Lista de caminhos protegidos do `automerge.yml` e o `CODEOWNERS` andam juntos.
- Release nunca usa cache de build e nunca publica direto como latest.
