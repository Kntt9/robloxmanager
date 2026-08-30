//! Lightweight i18n â€” string dictionary keyed by a `Language` stored in the
//! egui context, exactly like the theme (see [`crate::theme`]).
//!
//! # Usage
//!
//! ```ignore
//! let lang = ctx.lang();
//! ui.label(tr(lang, "accounts"));
//! ```
//!
//! New strings added to the UI must be added to [`en`] and [`pt_br`] so the
//! translator knows about them. Un-translated keys fall back to the English
//! value (the key itself is the English string).

use eframe::egui;

/// Languages the UI can be displayed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Language {
    /// English (default).
    #[serde(rename = "en")]
    En,
    /// Brazilian Portuguese.
    #[serde(rename = "pt-BR")]
    PtBr,
}

impl Language {
    /// Human-readable label for the language picker.
    pub fn label(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::PtBr => "Portugu\u{00EA}s (BR)",
        }
    }

    /// The persisted string form, mirroring the serde rename.
    pub fn as_str(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::PtBr => "pt-BR",
        }
    }

    /// Parse the persisted string form; anything unknown falls back to English.
    pub fn from_str(s: &str) -> Self {
        match s {
            "pt-BR" => Language::PtBr,
            _ => Language::En,
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::En
    }
}

// ---------------------------------------------------------------------------
// Storage in the egui context (same pattern as Theme)
// ---------------------------------------------------------------------------

const LANG_KEY: &str = "rm_language";

/// Install `lang` as the one the context uses. Called once at startup and
/// whenever the user changes the language in Settings.
pub fn install(ctx: &egui::Context, lang: Language) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(LANG_KEY), lang));
}

/// The language installed on `ctx`, or the default English one.
pub fn of(ctx: &egui::Context) -> Language {
    ctx.data(|d| d.get_temp::<Language>(egui::Id::new(LANG_KEY)))
        .unwrap_or_default()
}

/// Convenience trait so `ui.lang()` works at any call site.
pub trait LangUi {
    fn lang(&self) -> Language;
}

impl LangUi for egui::Ui {
    fn lang(&self) -> Language {
        of(self.ctx())
    }
}

impl LangUi for egui::Context {
    fn lang(&self) -> Language {
        of(self)
    }
}

// ---------------------------------------------------------------------------
// Dictionary
// ---------------------------------------------------------------------------

/// Look up a translation for `key` in `lang`. Falls back to English if the
/// key is not in the dictionary (the key itself is the English text).
///
/// Keys are string literals (`&'static str`), so the returned reference is
/// `'static` too. That keeps call sites free of the elided-lifetime dance that
/// closures returning `&str` trip over.
pub fn tr(lang: Language, key: &'static str) -> &'static str {
    // First try the target language, then fall back to English.
    match lang {
        Language::PtBr => pt_br(key).unwrap_or(en(key)),
        Language::En => en(key),
    }
}

/// English dictionary â€” returns the key itself (identity).
fn en(key: &'static str) -> &'static str {
    key
}

/// Brazilian Portuguese dictionary. Falls back to `None` for untranslated
/// keys, which the caller then passes through [`en`].
fn pt_br(key: &'static str) -> Option<&'static str> {
    Some(match key {
        // ---- Top bar tabs ----
        "Accounts" => "Contas",
        "Private Servers" => "Servidores Privados",
        "Presets" => "Predefini\u{00E7}\u{00F5}es",
        "Settings" => "Configura\u{00E7}\u{00F5}es",
        "Stats" => "Estat\u{00ED}sticas",
        "Games" => "Jogos",

        // ---- Games tab ----
        "Refresh" => "Atualizar",
        "Search games\u{2026}" => "Buscar jogos\u{2026}",
        "Popular" => "Populares",
        "Top Rated" => "Melhor Avaliados",
        "Top Earning" => "Mais Rent\u{00E1}veis",
        "No games found for that search." => "Nenhum jogo encontrado para essa busca.",
        "Search results ({} game)" => "Resultado da busca ({} jogo)",
        "Search results ({} games)" => "Resultados da busca ({} jogos)",
        "Remove from favorites" => "Remover dos favoritos",
        "Add to favorites" => "Adicionar aos favoritos",
        "Copy Place ID {}" => "Copiar Place ID {}",
        "Loading games\u{2026}" => "Carregando jogos\u{2026}",
        "Re-fetch all game feeds" => "Recarregar todas as listas",
        "Check your connection, then try Refresh." => "Verifique sua conex\u{00E3}o e tente Atualizar.",

        // ---- Stats tab ----
        "Statistics" => "Estat\u{00ED}sticas",
        "Add some accounts to see statistics here." => "Adicione contas para ver estat\u{00ED}sticas aqui.",
        "Presence" => "Presen\u{00E7}a",
        "Total" => "Total",
        "Online" => "Online",
        "In Game" => "No Jogo",
        "In Studio" => "No Studio",
        "Offline" => "Offline",
        "Moderated" => "Moderadas",
        "Cookie Expired" => "Cookie Expirado",
        "Roblox clients running:" => "Roblox em execu\u{00E7}\u{00E3}o:",
        "Accounts by Group" => "Contas por Grupo",
        "Split of accounts by current status" => "Divis\u{00E3}o das contas por status atual",
        "{}/{} online" => "{}/{} online",
        "Every account in RM" => "Todas as contas do RM",
        "Accounts with a running Roblox client" => "Contas com Roblox em execu\u{00E7}\u{00E3}o",
        "Accounts currently in a Roblox game" => "Contas atualmente em um jogo Roblox",
        "Accounts open in Roblox Studio" => "Contas abertas no Roblox Studio",
        "Accounts with no Roblox client running" => "Contas sem Roblox em execu\u{00E7}\u{00E3}o",
        "Accounts with an active moderation/termination" => "Contas com modera\u{00E7}\u{00E3}o/termina\u{00E7}\u{00E3}o ativa",
        "Accounts whose cookie no longer validates" => "Contas cujo cookie n\u{00E3}o valida mais",

        // ---- Account panel ----
        "Launch" => "Lan\u{00E7}ar",
        "Enter a place ID to launch this account" => "Insira um Place ID para lan\u{00E7}ar esta conta",
        "Place ID" => "Place ID",
        "Job ID (optional)" => "Job ID (opcional)",
        "Specific server GUID" => "GUID do servidor espec\u{00ED}fico",
        "Details" => "Detalhes",
        "Alias" => "Apelido",
        "Group" => "Grupo",
        "Validated" => "Validado",
        "Location" => "Localiza\u{00E7}\u{00E3}o",
        "Notes" => "Anota\u{00E7}\u{00F5}es",
        "Saved automatically when you click away." => "Salvo automaticamente ao clicar fora.",
        "Add notes about this account (origin, password hints, role\u{2026})" => "Anota\u{00E7}\u{00F5}es sobre esta conta (origem, dicas de senha, fun\u{00E7}\u{00E3}o\u{2026})",
        "Cookie expired. Remove and re-add this account with a fresh cookie." => "Cookie expirado. Remova e adicione esta conta novamente com um cookie novo.",
        "Remove account" => "Remover conta",
        "More actions" => "Mais a\u{00E7}\u{00F5}es",
        "Open a webview signed in as this account" => "Abrir navegador logado como esta conta",
        "Open browser as" => "Abrir como",
        "Save these inputs as a launch preset" => "Salvar estes dados como predefini\u{00E7}\u{00E3}o de lan\u{00E7}amento",
        "Save as preset" => "Salvar como predefini\u{00E7}\u{00E3}o",
        "Save" => "Salvar",
        "Cancel" => "Cancelar",
        "Preset name" => "Nome da predefini\u{00E7}\u{00E3}o",
        "Kill all running Roblox instances" => "Encerrar todas as inst\u{00E2}ncias do Roblox",
        "Enter a Place ID to launch" => "Insira um Place ID para lan\u{00E7}ar",
        "Launch this account into the chosen place" => "Lan\u{00E7}ar esta conta no jogo escolhido",

        // ---- Settings ----
        "Storage" => "Armazenamento",
        "Launch Behavior" => "Comportamento de Lan\u{00E7}amento",
        "Use Windows Credential Manager (instead of encrypted file)" => "Usar o Gerenciador de Credenciais do Windows (em vez de arquivo criptografado)",
        "Enable multi-instance" => "Habilitar multi-inst\u{00E2}ncia",
        "Close all Roblox processes (including tray) before enabling." => "Feche todos os processos do Roblox (incluindo a bandeja) antes de habilitar.",
        "Kill Roblox tray/background processes automatically" => "Encerrar processos de bandeja/fundo do Roblox automaticamente",
        "Kills idle \"always running\" Roblox processes (--launch-to-tray)." => "Encerra processos Roblox \"sempre em execu\u{00E7}\u{00E3}o\" ociosos (--launch-to-tray).",
        "Auto-arrange Roblox windows after launch" => "Organizar janelas do Roblox automaticamente ap\u{00F3}s o lan\u{00E7}amento",
        "Tiles Roblox windows in a grid (2 = side-by-side, 4 = 2\u{00D7}2, etc.)." => "Organiza as janelas do Roblox em grade (2 = lado a lado, 4 = 2\u{00D7}2, etc.).",
        "Name Roblox windows after their account" => "Nomear janelas do Roblox pelo nome da conta",
        "Window titles are readable by any program, and show up in screenshots and streams." => "Os t\u{00ED}tulos das janelas s\u{00E3}o leg\u{00ED}veis por qualquer programa e aparecem em capturas e transmiss\u{00F5}es.",
        "Launch delay:" => "Atraso de lan\u{00E7}amento:",
        "Minimum gap between account launches. Applies to single and bulk launches. 0 disables throttling." => "Intervalo m\u{00ED}nimo entre lan\u{00E7}amentos de contas. Aplica-se a lan\u{00E7}amentos \u{00FA}nicos e em massa. 0 desativa a limita\u{00E7}\u{00E3}o.",
        "Privacy" => "Privacidade",
        "Clear RobloxCookies.dat before each launch" => "Limpar RobloxCookies.dat antes de cada lan\u{00E7}amento",
        "Prevents Roblox from associating your accounts via stored cookies." => "Impede que o Roblox associe suas contas por meio de cookies armazenados.",
        "Anonymize account names" => "Anonimizar nomes de contas",
        "Replaces usernames and display names with generic \"Account 1\", \"Account 2\", etc." => "Substitui nomes de usu\u{00E1}rio e nomes de exibi\u{00E7}\u{00E3}o por gen\u{00E9}ricos como \"Conta 1\", \"Conta 2\", etc.",
        "Developer Options" => "Op\u{00E7}\u{00F5}es de Desenvolvedor",
        "Show the Asset Manager tab" => "Mostrar a aba Gerenciador de Assets",
        "Upload assets to Roblox from any saved account, track moderation, and grant experiences permission to use them." => "Envie assets para o Roblox de qualquer conta salva, acompanhe a modera\u{00E7}\u{00E3}o e conceda permiss\u{00F5}es para experi\u{00EA}ncias usarem.",
        "Roblox Player Path" => "Caminho do Roblox Player",
        "Leave empty for auto-detect:" => "Deixe vazio para detectar automaticamente:",
        "Backup & Transfer" => "Backup e Transfer\u{00EA}ncia",
        "Export your accounts to a portable file. Cookies are included in plaintext, so treat the file like a password manager backup." => "Exporte suas contas para um arquivo port\u{00E1}til. Os cookies est\u{00E3}o em texto puro, ent\u{00E3}o trate o arquivo como um backup de gerenciador de senhas.",
        "Export accounts\u{2026}" => "Exportar contas\u{2026}",
        "Import accounts\u{2026}" => "Importar contas\u{2026}",
        "Language" => "Idioma",
        "English" => "English",
        "Portugu\u{00EA}s (BR)" => "Portugu\u{00EA}s (BR)",
        "Encryption" => "Criptografia",
        "Accounts are encrypted with your master password." => "As contas s\u{00E3}o criptografadas com sua senha mestra.",
        "RM asks for it every time it starts. If you forget it, the accounts cannot be recovered." => "O RM pede a senha sempre que inicia. Se voc\u{00EA} esquec\u{00EA}-la, as contas n\u{00E3}o podem ser recuperadas.",
        "Accounts are encrypted and unlock automatically on this PC." => "As contas s\u{00E3}o criptografadas e desbloqueiam automaticamente neste PC.",
        "The key is held in Windows Credential Manager, so the file is useless on its own. Anything running as you can still read it." => "A chave fica no Gerenciador de Credenciais do Windows, ent\u{00E3}o o arquivo sozinho n\u{00E3}o vale nada. Qualquer programa rodando como voc\u{00EA} ainda pode l\u{00EA}-lo.",
        "Change your master password:" => "Alterar sua senha mestra:",
        "Require a master password at startup:" => "Exigir senha mestra na inicializa\u{00E7}\u{00E3}o:",
        "New password" => "Nova senha",
        "Confirm password" => "Confirmar senha",
        "Passwords do not match." => "As senhas n\u{00E3}o coincidem.",
        "Stop asking for a password" => "Parar de pedir a senha",
        "Save Settings" => "Salvar Configura\u{00E7}\u{00F5}es",
        "Change password" => "Alterar senha",
        "Set password" => "Definir senha",
        "RM asks for it every time it starts. If you forget it, the accounts cannot be recovered." => "O RM pede a senha sempre que inicia. Se voc\u{00EA} esquec\u{00EA}-la, as contas n\u{00E3}o podem ser recuperadas.",
        "The key is held in Windows Credential Manager, so the file is useless on its own. Anything running as you can still read it." => "A chave fica no Gerenciador de Credenciais do Windows, ent\u{00E3}o o arquivo sozinho n\u{00E3}o vale nada. Qualquer programa rodando como voc\u{00EA} ainda pode l\u{00EA}-lo.",
        "Renames each launched Roblox window to the account's alias, so tiled windows are tellable apart.\n\nOff by default. This writes to the Roblox window rather than just reading it, and how Hyperion treats that is not documented. It also changes what capture software matching on window title will find." => "Renomeia cada janela do Roblox lan\u{00E7}ada para o apelido da conta, para que as janelas organizadas sejam distingu\u{00ED}veis.\n\nDesativado por padr\u{00E3}o. Isso grava na janela do Roblox, em vez de apenas l\u{00EA}-la, e como o Hyperion trata isso n\u{00E3}o \u{00E9} documentado. Tamb\u{00E9}m altera o que o software de captura que usa o t\u{00ED}tulo da janela encontrar\u{00E1}.",
        "\u{26a0} Uploads are permanent and public. Every asset is moderated under the account that uploaded it." => "\u{26a0} Os uploads s\u{00E3}o permanentes e p\u{00FA}blicos. Cada asset \u{00E9} moderado na conta que o enviou.",
        "(Roblox rate-limits some IPs)" => "(o Roblox limita algumas IPs)",
        "\u{26a0} This interacts with Hyperion anti-cheat and may carry ban risk." => "\u{26a0} Isso interage com o anti-cheat Hyperion e pode trazer risco de banimento.",
        "\u{26a0} Recommended when multi-instance is enabled. Tray processes stack up." => "\u{26a0} Recomendado quando a multi-inst\u{00E2}ncia est\u{00E1} habilitada. Processos de bandeja se acumulam.",

        // ---- Group panel (bulk launch) ----
        "{} Accounts Selected" => "{} Contas Selecionadas",
        "Clear selection" => "Limpar sele\u{00E7}\u{00E3}o",
        "Bulk Launch" => "Lan\u{00E7}amento em Massa",
        "All selected accounts will join the same server sequentially." => "Todas as contas selecionadas entrar\u{00E3}o no mesmo servidor, em sequ\u{00EA}ncia.",
        "Launch {} Accounts" => "Lan\u{00E7}ar {} Contas",
        "Kill All Instances" => "Encerrar Todas as Inst\u{00E2}ncias",
        "Account {}" => "Conta {}",
        "Launch all" => "Lan\u{00E7}ar todos",
        "Kill all" => "Encerrar todos",
        "Place ID:" => "Place ID:",
        "Job ID (optional):" => "Job ID (opcional):",

        // ---- Presets panel ----
        "Edit Preset" => "Editar Predefini\u{00E7}\u{00E3}o",
        "New Preset" => "Nova Predefini\u{00E7}\u{00E3}o",
        "Reload" => "Recarregar",
        "Re-scan the presets folder for new or removed presets" => "Revarrer a pasta de predefini\u{00E7}\u{00F5}es em busca de novas ou removidas",
        "Open folder" => "Abrir pasta",
        "Reveal the presets folder in Explorer" => "Revelar a pasta de predefini\u{00E7}\u{00F5}es no Explorer",
        "Name:" => "Nome:",
        "e.g. Adopt Me" => "ex.: Adopt Me",
        "e.g. 920587237" => "ex.: 920587237",
        "Save changes" => "Salvar altera\u{00E7}\u{00F5}es",
        "+ Add Preset" => "+ Adicionar Predefini\u{00E7}\u{00E3}o",
        "Place ID must be a number." => "O Place ID deve ser um n\u{00FA}mero.",
        "Saved Presets" => "Predefini\u{00E7}\u{00F5}es Salvas",
        "No presets yet. Create one above to launch favorite games faster." => "Nenhuma predefini\u{00E7}\u{00E3}o ainda. Crie uma acima para lan\u{00E7}ar jogos favoritos mais r\u{00E1}pido.",
        "Delete" => "Excluir",
        "Edit" => "Editar",

        // ---- Presets continued ----
        "Place {}, Job {}" => "Place {}, Job {}",
        "Place {}" => "Place {}",

        // ---- Sidebar ----
        "Search accounts\u{2026}" => "Buscar contas\u{2026}",
        "Add Account" => "Adicionar Conta",
        "{} selected" => "{} selecionadas",

        // ---- Toasts / notifications ----
        "Preset saved" => "Predefini\u{00E7}\u{00E3}o salva",
        "Preset deleted" => "Predefini\u{00E7}\u{00E3}o exclu\u{00ED}da",
        "Save failed: {e}" => "Falha ao salvar: {e}",
        "Delete failed: {e}" => "Falha ao excluir: {e}",
        "Settings saved" => "Configura\u{00E7}\u{00F5}es salvas",
        "Game launched" => "Jogo lan\u{00E7}ado",
        "Copied to clipboard" => "Copiado para a \u{00E1}rea de transfer\u{00EA}ncia",
        "Refreshing all accounts\u{2026}" => "Atualizando todas as contas\u{2026}",
        "Could not create folder: {e}" => "N\u{00E3}o foi poss\u{00ED}vel criar a pasta: {e}",
        "Skipped {} unreadable preset file(s)" => "{} arquivo(s) de predefini\u{00E7}\u{00E3}o ileg\u{00ED}vel(is) ignorado(s)",
        "Failed to load presets: {e}" => "Falha ao carregar predefini\u{00E7}\u{00F5}es: {e}",
        "Killed {count} instance(s)" => "{count} inst\u{00E2}ncia(s) encerrada(s)",
        "Multi-instance enabled" => "Multi-inst\u{00E2}ncia habilitada",
        "Multi-instance disabled (takes effect after restart)" => "Multi-inst\u{00E2}ncia desabilitada (entra em vigor ap\u{00F3}s reiniciar)",
        "Failed: {e}" => "Falhou: {e}",

        // ---- Launch panel ----

        // ---- Short messages ----
        "Copied Place ID {}" => "Place ID {} copiado",
        "Exported {n} account(s) to {}" => "{n} conta(s) exportada(s) para {}",
        "Export failed: {e}" => "Exporta\u{00E7}\u{00E3}o falhou: {e}",
        "Imported {n} account(s) ({skipped} already present, skipped)" => "{n} conta(s) importada(s) ({skipped} j\u{00E1} existentes, puladas)",
        "Import failed: {e}" => "Importa\u{00E7}\u{00E3}o falhou: {e}",
        "No accounts to export" => "Nenhuma conta para exportar",
        "Unlock the account store first" => "Destrave o armazenamento de contas primeiro",
        "No account selected" => "Nenhuma conta selecionada",
        "Pick an account in the sidebar to view it." => "Escolha uma conta na barra lateral para visualiz\u{00E1}-la.",
        "Manage launch presets" => "Gerenciar predefini\u{00E7}\u{00F5}es de lan\u{00E7}amento",
        "Game not found for this Place ID." => "Jogo n\u{00E3}o encontrado para este Place ID.",
        "Ready to launch" => "Pronto para lan\u{00E7}ar",
        "Identifying game\u{2026}" => "Identificando jogo\u{2026}",

        // ---- Exploits tab ----
        "Exploits" => "Exploits",
        "Refresh" => "Atualizar",
        "Re-fetch exploit statuses from WEAO" => "Recarregar status dos exploits do WEAO",
        "Updated" => "Atualizado",
        "Outdated" => "Desatualizado",
        "Executor is up to date" => "Executor est\u{00E1} atualizado",
        "Executor is outdated" => "Executor est\u{00E1} desatualizado",
        "Loading exploits\u{2026}" => "Carregando exploits\u{2026}",
        "No exploits found." => "Nenhum exploit encontrado.",
        "Check your connection, then try Refresh." => "Verifique sua conex\u{00E3}o e tente Atualizar.",
        "Paid" => "Pagos",
        "Free" => "Gratuitos",
        "External" => "Externos",
        "Detected" => "Detectado",
        "Undetected" => "N\u{00E3}o detectado",
        "Visit" => "Acessar",
        "View disclaimer" => "Ver aviso",
        "You have not yet accepted the disclaimer for this area." => "Voc\u{00EA} ainda n\u{00E3}o aceitou o aviso para esta \u{00E1}rea.",
        "The Exploits area is locked until you accept the disclaimer." => "A \u{00E1}rea de Exploits est\u{00E1} bloqueada at\u{00E9} que voc\u{00EA} aceite o aviso.",
        "View terms" => "Ver termos",
        "Review the disclaimer" => "Rever o aviso",
        "Agree" => "Concordar",
        "Back" => "Voltar",
        "N/A" => "N/D",

        // ---- Disclaimer modal ----
        "Warning - Exploits" => "Aviso - Exploits",
        "Exploits disclaimer body" => "Antes de utilizar esta \u{00E1}rea, leia atentamente:\n\n\u{2022} Esta \u{00E1}rea cont\u{00E9}m informa\u{00E7}\u{00F5}es sobre ferramentas de terceiros usadas para explora\u{00E7}\u{00E3}o/executors no Roblox.\n\u{2022} O uso dessas ferramentas pode violar as regras/Termos do Roblox.\n\u{2022} Existe risco de puni\u{00E7}\u{00E3}o, incluindo advert\u{00EA}ncia, suspens\u{00E3}o ou banimento da conta.\n\u{2022} O status \u{201C}Atualizado\u{201D} n\u{00E3}o significa que a ferramenta seja segura ou indetect\u{00E1}vel.\n\u{2022} Ferramentas de terceiros podem apresentar riscos de seguran\u{00E7}a.\n\u{2022} Links externos devem ser tratados com cautela.\n\u{2022} O Roblox Manager n\u{00E3}o garante o funcionamento, seguran\u{00E7}a ou aus\u{00EA}ncia de puni\u{00E7}\u{00E3}o.\n\u{2022} O uso dessas ferramentas \u{00E9} de responsabilidade do usu\u{00E1}rio.\n\nAo concordar, voc\u{00EA} assume total responsabilidade pelo uso desta \u{00E1}rea.",
        "I understand the risks and want to access the Exploits area." => "Eu entendo os riscos e quero acessar a \u{00E1}rea de Exploits.",
        "Continue" => "Continuar",

        // ---- Trash ----
        "Move account to trash" => "Mover conta para a lixeira",
        "Trash" => "Lixeira",
        "Restore" => "Restaurar",
        "Delete permanently" => "Excluir permanentemente",
        "Empty trash" => "Esvaziar lixeira",
        "No accounts in the trash." => "N\u{00E3}o h\u{00E1} contas na lixeira.",
        "Move account to trash?" => "Mover conta para a lixeira?",
        "The account can be restored later." => "A conta poder\u{00E1} ser restaurada posteriormente.",
        "This account is running. Are you sure you want to move it to the trash?" => "Esta conta est\u{00E1} em execu\u{00E7}\u{00E3}o. Tem certeza que deseja mov\u{00EA}-la para a lixeira?",
        "Move to trash" => "Mover para a lixeira",
        "This action cannot be undone." => "Esta a\u{00E7}\u{00E3}o n\u{00E3}o poder\u{00E1} ser desfeita.",
        "Delete all trashed accounts permanently?" => "Excluir permanentemente todas as contas da lixeira?",
        "Recently removed" => "Removida recentemente",
        "Restored account" => "Conta restaurada",
        "Moved {} account(s) to trash" => "{n} conta(s) movida(s) para a lixeira",
        "Account permanently deleted" => "Conta exclu\u{00ED}da permanentemente",
        "Trash emptied" => "Lixeira esvaziada",

        // ---- Servers panel ----
        "Servers" => "Servidores",
        "Game" => "Jogo",
        "No game selected" => "Nenhum jogo selecionado",
        "Enter a Place ID to view servers." => "Informe um Place ID para visualizar os servidores.",
        "Sort by" => "Ordenar por",
        "Lowest ping" => "Menor ping",
        "Emptiest" => "Mais vazios",
        "Fewest players" => "Menos jogadores",
        "Most free slots" => "Mais vagas",
        "Hide full servers" => "Ocultar servidores cheios",
        "Featured servers" => "Servidores em destaque",
        "is here" => "est\u{00E1} aqui",
        "Join" => "Entrar",
        "Available" => "Dispon\u{00ED}vel",
        "Full" => "Cheio",
        "players" => "jogadores",
        "Load more" => "Carregar mais",
        "Loading servers\u{2026}" => "Carregando servidores\u{2026}",
        "No servers available at the moment." => "Nenhum servidor dispon\u{00ED}vel no momento.",
        "Try again" => "Tentar novamente",
        "Re-fetch servers" => "Recarregar servidores",
        "Server" => "Servidor",
        "Move account to trash" => "Mover conta para a lixeira",
        "Open servers panel" => "Abrir painel de servidores",
        "Rate limited. Try again in a moment." => "Limite de requisi\u{00E7}\u{00F5}es atingido. Tente novamente em instantes.",

        _ => return None,
    })
}
