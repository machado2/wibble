use serde_json::{json, Value};

use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};
use crate::services::article_jobs::{
    ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, ARTICLE_JOB_PHASE_QUEUED,
    ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RENDERING_IMAGES,
    ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_TRANSLATING, ARTICLE_JOB_PHASE_WRITING,
};
use crate::services::article_language::PreferredLanguageSource;

const ENGLISH_LANGUAGE_CODE: &str = "en";
const PORTUGUESE_LANGUAGE_CODE: &str = "pt";

#[derive(Clone, Copy, Debug)]
pub struct SiteText {
    language: SupportedTranslationLanguage,
}

pub fn default_site_language() -> SupportedTranslationLanguage {
    find_supported_translation_language(ENGLISH_LANGUAGE_CODE)
        .expect("English must remain supported for site chrome")
}

pub fn resolve_site_language(
    browser_language: Option<SupportedTranslationLanguage>,
) -> SupportedTranslationLanguage {
    match browser_language {
        Some(language) if language.code == PORTUGUESE_LANGUAGE_CODE => language,
        _ => default_site_language(),
    }
}

pub fn site_text(language: SupportedTranslationLanguage) -> SiteText {
    SiteText { language }
}

impl SiteText {
    pub fn language(self) -> SupportedTranslationLanguage {
        self.language
    }

    pub fn template_strings(self) -> Value {
        if self.is_portuguese() {
            portuguese_template_strings()
        } else {
            english_template_strings()
        }
    }

    pub fn index_meta_title(self, search: Option<&str>) -> String {
        match search.filter(|value| !value.trim().is_empty()) {
            Some(search) if self.is_portuguese() => {
                format!("Resultados da busca por {}", search)
            }
            Some(search) => format!("Search results for {}", search),
            None if self.is_portuguese() => "Últimas notícias do Wibble".to_string(),
            None => "Latest Wibble News".to_string(),
        }
    }

    pub fn index_meta_description(self) -> &'static str {
        if self.is_portuguese() {
            "Boletins oficiais sisudos sobre confusão cívica, excesso institucional e emergências evitáveis."
        } else {
            "Dry official bulletins on civic confusion, institutional overreaction, and preventable emergencies."
        }
    }

    pub fn sort_label_newest(self) -> &'static str {
        if self.is_portuguese() {
            "Mais recentes"
        } else {
            "Newest"
        }
    }

    pub fn sort_label_hot(self) -> &'static str {
        if self.is_portuguese() {
            "Em alta"
        } else {
            "Hot"
        }
    }

    pub fn time_label_any(self) -> &'static str {
        if self.is_portuguese() {
            "Qualquer período"
        } else {
            "Any time"
        }
    }

    pub fn time_label_week(self) -> &'static str {
        if self.is_portuguese() {
            "Esta semana"
        } else {
            "This week"
        }
    }

    pub fn time_label_month(self) -> &'static str {
        if self.is_portuguese() {
            "Este mês"
        } else {
            "This month"
        }
    }

    pub fn create_meta_title(self) -> &'static str {
        if self.is_portuguese() {
            "Criar um novo artigo"
        } else {
            "Create a new article"
        }
    }

    pub fn create_meta_description(self) -> &'static str {
        if self.is_portuguese() {
            "Envie um briefing e deixe o The Wibble redigir uma reportagem satírica em tom sisudo."
        } else {
            "Submit a brief and let The Wibble draft a straight-faced satirical report."
        }
    }

    pub fn create_prompt_presets(self) -> [(&'static str, &'static str); 4] {
        if self.is_portuguese() {
            [
                (
                    "Memorando de política",
                    "Um ministério nacional dos transportes começa a publicar boletins de prontidão emocional junto com avisos de atraso, e empregadores passam a exigir que funcionários os anexem aos pedidos de licença.",
                ),
                (
                    "Mesa cívica",
                    "Um conselho municipal abre um inquérito formal depois que um pombo incomumente competente é visto repetidas vezes orientando o tráfego de pedestres melhor do que a sinalização atual.",
                ),
                (
                    "Tribunal esportivo",
                    "Uma federação de futebol divulga uma revisão de conformidade depois que todas as entrevistas pós-jogo começam a soar como teleconferências trimestrais de resultados e torcedores passam a exigir orientações mais claras.",
                ),
                (
                    "Briefing de pesquisa",
                    "Um instituto respeitado publica um relatório sóbrio concluindo que o humor nacional é melhor classificado como 'administrável, com pepitas', provocando interesse parlamentar imediato.",
                ),
            ]
        } else {
            [
                (
                    "Policy Memo",
                    "A national transport ministry begins issuing emotional readiness bulletins alongside delay notices, and employers start asking staff to attach them to leave requests.",
                ),
                (
                    "Civic Desk",
                    "A borough council opens a formal inquiry after one unusually competent pigeon is repeatedly observed directing pedestrian traffic more effectively than the current signage.",
                ),
                (
                    "Sports Tribunal",
                    "A football federation releases a compliance review after every post-match interview starts sounding like a quarterly earnings call and supporters begin demanding clearer guidance.",
                ),
                (
                    "Research Brief",
                    "A respected institute publishes a sober report concluding that the national mood is best classified as 'manageable, with nuggets', prompting immediate parliamentary interest.",
                ),
            ]
        }
    }

    pub fn wait_meta_title(self) -> &'static str {
        if self.is_portuguese() {
            "Gerando artigo"
        } else {
            "Generating article"
        }
    }

    pub fn wait_meta_description(self) -> &'static str {
        if self.is_portuguese() {
            "O artigo ainda está sendo gerado e esta página se atualiza automaticamente."
        } else {
            "The article is still being generated and this page auto-refreshes."
        }
    }

    pub fn wait_publication_copy(self, is_logged_in: bool) -> (String, String) {
        if self.is_portuguese() {
            if is_logged_in {
                (
                    "Destino: rascunho".to_string(),
                    "Artigos de usuários logados permanecem privados até você revisar e publicar."
                        .to_string(),
                )
            } else {
                (
                    "Destino: público".to_string(),
                    "Artigos anônimos são publicados imediatamente e não ficam vinculados a uma conta editável."
                        .to_string(),
                )
            }
        } else if is_logged_in {
            (
                "Destination: draft".to_string(),
                "Signed-in articles stay private until you review and publish them.".to_string(),
            )
        } else {
            (
                "Destination: public".to_string(),
                "Anonymous articles publish immediately and are not tied to an editable owner account."
                    .to_string(),
            )
        }
    }

    pub fn wait_stage_copy(self, phase: Option<&str>) -> (String, String) {
        match phase.unwrap_or(ARTICLE_JOB_PHASE_WRITING) {
            ARTICLE_JOB_PHASE_QUEUED => {
                if self.is_portuguese() {
                    (
                        "Na fila para geração".to_string(),
                        "O prompt está aguardando uma vaga antes de a redação começar.".to_string(),
                    )
                } else {
                    (
                        "Queued for generation".to_string(),
                        "The prompt is waiting for a generation slot before drafting starts."
                            .to_string(),
                    )
                }
            }
            ARTICLE_JOB_PHASE_RESEARCHING => {
                if self.is_portuguese() {
                    (
                        "Pesquisando o briefing".to_string(),
                        "O trabalho está reunindo contexto limitado antes de o rascunho ser escrito."
                            .to_string(),
                    )
                } else {
                    (
                        "Researching the brief".to_string(),
                        "The job is gathering bounded context before the draft is written."
                            .to_string(),
                    )
                }
            }
            ARTICLE_JOB_PHASE_TRANSLATING => {
                if self.is_portuguese() {
                    (
                        "Traduzindo o rascunho".to_string(),
                        "O texto do artigo está sendo convertido para uma nova variante de idioma."
                            .to_string(),
                    )
                } else {
                    (
                        "Translating the draft".to_string(),
                        "The article text is being transformed into a new language variant."
                            .to_string(),
                    )
                }
            }
            ARTICLE_JOB_PHASE_AWAITING_USER_INPUT => {
                if self.is_portuguese() {
                    (
                        "Aguardando esclarecimento".to_string(),
                        "O rascunho está pausado porque o briefing ainda é ambíguo o bastante para mudar materialmente o artigo."
                            .to_string(),
                    )
                } else {
                    (
                        "Waiting for clarification".to_string(),
                        "The draft is paused because the brief is still ambiguous enough to change the article materially.".to_string(),
                    )
                }
            }
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW => {
                if self.is_portuguese() {
                    (
                        "Preparando revisão".to_string(),
                        "O rascunho está sendo empacotado para uma última passada de revisão."
                            .to_string(),
                    )
                } else {
                    (
                        "Preparing review".to_string(),
                        "The draft is being packaged for a final review pass.".to_string(),
                    )
                }
            }
            ARTICLE_JOB_PHASE_RENDERING_IMAGES => {
                if self.is_portuguese() {
                    (
                        "Renderizando ilustrações".to_string(),
                        "O rascunho da matéria está pronto e a fila de imagens está renderizando a arte."
                            .to_string(),
                    )
                } else {
                    (
                        "Rendering illustrations".to_string(),
                        "The story draft is ready and the image queue is actively rendering art."
                            .to_string(),
                    )
                }
            }
            _ => {
                if self.is_portuguese() {
                    (
                        "Redigindo a matéria".to_string(),
                        "A manchete, o ângulo e o corpo do artigo ainda estão sendo montados."
                            .to_string(),
                    )
                } else {
                    (
                        "Drafting the story".to_string(),
                        "The headline, angle, and article body are still being assembled."
                            .to_string(),
                    )
                }
            }
        }
    }

    pub fn wait_image_stage_copy(
        self,
        image_total: usize,
        image_completed: usize,
        image_processing: usize,
        image_failed: usize,
        markdown_ready: bool,
    ) -> (String, String) {
        if image_total == 0 && !markdown_ready {
            self.wait_stage_copy(Some(ARTICLE_JOB_PHASE_WRITING))
        } else if image_total == 0 {
            if self.is_portuguese() {
                (
                    "Preparando o artigo".to_string(),
                    "O rascunho da matéria está pronto e a página está sendo finalizada."
                        .to_string(),
                )
            } else {
                (
                    "Preparing the article".to_string(),
                    "The story draft is ready and the page is being finalized.".to_string(),
                )
            }
        } else if image_processing > 0 {
            self.wait_stage_copy(Some(ARTICLE_JOB_PHASE_RENDERING_IMAGES))
        } else if image_failed > 0 && image_completed < image_total {
            if self.is_portuguese() {
                (
                    "Recuperando o conjunto de imagens".to_string(),
                    "Algumas ilustrações falharam e o artigo está aguardando os resultados restantes."
                        .to_string(),
                )
            } else {
                (
                    "Recovering the image set".to_string(),
                    "Some illustrations failed and the article is waiting on the remaining results."
                        .to_string(),
                )
            }
        } else if self.is_portuguese() {
            (
                "Finalizando o artigo".to_string(),
                "O rascunho está completo e a página está prestes a entrar no ar.".to_string(),
            )
        } else {
            (
                "Finalizing the article".to_string(),
                "The draft is complete and the page is about to go live.".to_string(),
            )
        }
    }

    pub fn wait_phase_label(self, phase: &str) -> &'static str {
        match phase {
            ARTICLE_JOB_PHASE_QUEUED => {
                if self.is_portuguese() {
                    "Fila"
                } else {
                    "Queued"
                }
            }
            ARTICLE_JOB_PHASE_AWAITING_USER_INPUT => {
                if self.is_portuguese() {
                    "Esclarecer"
                } else {
                    "Clarify"
                }
            }
            ARTICLE_JOB_PHASE_RENDERING_IMAGES => {
                if self.is_portuguese() {
                    "Imagens"
                } else {
                    "Images"
                }
            }
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW => {
                if self.is_portuguese() {
                    "Revisar"
                } else {
                    "Review"
                }
            }
            _ => {
                if self.is_portuguese() {
                    "Escrever"
                } else {
                    "Write"
                }
            }
        }
    }

    pub fn wait_clarification_deadline_note(self, deadline: &str) -> String {
        if self.is_portuguese() {
            format!(
                "Se ninguém responder até {}, o trabalho continua com uma alternativa conservadora.",
                deadline
            )
        } else {
            format!(
                "If nobody answers by {}, the job resumes with a conservative fallback.",
                deadline
            )
        }
    }

    pub fn server_error_title(self) -> &'static str {
        if self.is_portuguese() {
            "Erro do servidor"
        } else {
            "Server error"
        }
    }

    pub fn server_error_description(self) -> &'static str {
        if self.is_portuguese() {
            "Ocorreu um erro inesperado do servidor ao carregar esta página."
        } else {
            "An unexpected server error occurred while loading this page."
        }
    }

    pub fn server_error_message(self) -> &'static str {
        if self.is_portuguese() {
            "Algo deu errado. Tente novamente mais tarde."
        } else {
            "Oops! Something went wrong. Please try again later."
        }
    }

    pub fn not_found_title(self) -> &'static str {
        if self.is_portuguese() {
            "Página não encontrada"
        } else {
            "Page not found"
        }
    }

    pub fn not_found_description(self) -> &'static str {
        if self.is_portuguese() {
            "A página solicitada não foi encontrada."
        } else {
            "The requested page could not be found."
        }
    }

    pub fn not_found_message(self) -> &'static str {
        if self.is_portuguese() {
            "A página que você está procurando não existe."
        } else {
            "The page you are looking for does not exist."
        }
    }

    pub fn image_gallery_meta_title(self) -> &'static str {
        if self.is_portuguese() {
            "Galeria de imagens geradas"
        } else {
            "Generated image gallery"
        }
    }

    pub fn image_gallery_meta_description(self) -> &'static str {
        if self.is_portuguese() {
            "Uma galeria navegável de imagens geradas por IA usadas nas histórias do Wibble."
        } else {
            "A browsable gallery of AI-generated images used in Wibble stories."
        }
    }

    pub fn image_info_description(self) -> &'static str {
        if self.is_portuguese() {
            "Detalhes de geração de uma imagem usada em um artigo do Wibble."
        } else {
            "Generation details for an image used in a Wibble article."
        }
    }

    pub fn edit_meta_title(self, article_title: &str) -> String {
        if self.is_portuguese() {
            format!("Editar: {}", article_title)
        } else {
            format!("Edit: {}", article_title)
        }
    }

    pub fn edit_preview_meta_title(self, article_title: &str) -> String {
        if self.is_portuguese() {
            format!("Prévia da edição com agente: {}", article_title)
        } else {
            format!("Agent edit preview: {}", article_title)
        }
    }

    pub fn image_status_label(self, status: &str) -> &'static str {
        match status {
            "pending" => {
                if self.is_portuguese() {
                    "Na fila"
                } else {
                    "Queued"
                }
            }
            "processing" => {
                if self.is_portuguese() {
                    "Gerando"
                } else {
                    "Generating"
                }
            }
            "failed" => {
                if self.is_portuguese() {
                    "Falhou"
                } else {
                    "Failed"
                }
            }
            "completed" => {
                if self.is_portuguese() {
                    "Pronta"
                } else {
                    "Ready"
                }
            }
            _ => {
                if self.is_portuguese() {
                    "Desconhecido"
                } else {
                    "Unknown"
                }
            }
        }
    }

    pub fn image_status_note(self, status: &str) -> &'static str {
        match status {
            "pending" => {
                if self.is_portuguese() {
                    "Uma nova renderização foi enfileirada a partir do prompt salvo."
                } else {
                    "A fresh render is queued from the saved prompt."
                }
            }
            "processing" => {
                if self.is_portuguese() {
                    "Uma nova renderização está em andamento a partir do prompt salvo."
                } else {
                    "A fresh render is in progress from the saved prompt."
                }
            }
            "failed" => {
                if self.is_portuguese() {
                    "A última tentativa de geração falhou. Você pode tentar de novo ou enviar uma substituição."
                } else {
                    "The last generation attempt failed. You can retry or upload a replacement."
                }
            }
            "completed" => {
                if self.is_portuguese() {
                    "Imagem atual armazenada. Você pode enviar uma substituição ou regenerar a partir do prompt salvo."
                } else {
                    "Current stored image. You can upload a replacement or regenerate from the saved prompt."
                }
            }
            _ => {
                if self.is_portuguese() {
                    "O status da imagem está disponível para este slot."
                } else {
                    "Image status is available for this slot."
                }
            }
        }
    }

    pub fn article_language_automatic_note(
        self,
        browser_language: Option<SupportedTranslationLanguage>,
        source_language: SupportedTranslationLanguage,
    ) -> String {
        if self.is_portuguese() {
            browser_language
                .map(|language| {
                    format!(
                        "Padrão do navegador: {}",
                        self.translation_language_name(language)
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "Edição original: {}",
                        self.translation_language_name(source_language)
                    )
                })
        } else {
            browser_language
                .map(|language| format!("Browser default: {}", language.name))
                .unwrap_or_else(|| format!("Original edition: {}", source_language.name))
        }
    }

    pub fn article_language_automatic_label(self) -> &'static str {
        if self.is_portuguese() {
            "Automático"
        } else {
            "Automatic"
        }
    }

    pub fn article_language_original_label(self, source_language_name: &str) -> String {
        if self.is_portuguese() {
            format!("Original ({})", source_language_name)
        } else {
            format!("Original ({})", source_language_name)
        }
    }

    pub fn article_language_original_note(self) -> &'static str {
        if self.is_portuguese() {
            "Edição-fonte manual"
        } else {
            "Manual source edition"
        }
    }

    pub fn article_language_requested_note(self, served_language_name: &str) -> String {
        if self.is_portuguese() {
            format!("Solicitado; exibindo {} por enquanto", served_language_name)
        } else {
            format!("Requested; showing {} for now", served_language_name)
        }
    }

    pub fn article_language_saved_note(self) -> &'static str {
        if self.is_portuguese() {
            "Salvo para este artigo"
        } else {
            "Saved for this article"
        }
    }

    pub fn article_language_selected_note(self) -> &'static str {
        if self.is_portuguese() {
            "Edição selecionada"
        } else {
            "Selected edition"
        }
    }

    pub fn article_language_open_when_available(self) -> &'static str {
        if self.is_portuguese() {
            "Abrir quando disponível"
        } else {
            "Open when available"
        }
    }

    pub fn article_research_mode_label(self, mode: Option<&str>) -> &'static str {
        match mode {
            Some("manual") if self.is_portuguese() => "Mesa de pesquisa solicitada",
            Some("manual") => "Requested research desk",
            _ if self.is_portuguese() => "Mesa de pesquisa automática",
            _ => "Automatic research desk",
        }
    }

    pub fn article_research_note(self, mode_label: &str, source_count: usize) -> String {
        if self.is_portuguese() {
            format!(
                "{}. Este registro foi fundamentado em {} resumo{} de fonte pública antes da redação. O rastro das fontes fica fora da página para preservar o tom sisudo.",
                mode_label,
                source_count,
                if source_count == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "{}. This filing was grounded against {} public-source brief{} before drafting. The source trace is kept off-page so the article body stays deadpan.",
                mode_label,
                source_count,
                if source_count == 1 { "" } else { "s" }
            )
        }
    }

    pub fn article_language_summary_note(
        self,
        preferred_language_source: PreferredLanguageSource,
        translation_requested: bool,
        translation_available: bool,
        preferred_language_name: &str,
    ) -> String {
        if translation_requested && !translation_available {
            if self.is_portuguese() {
                format!("Solicitado {}", preferred_language_name)
            } else {
                format!("Requested {}", preferred_language_name)
            }
        } else {
            match preferred_language_source {
                PreferredLanguageSource::Explicit => {
                    if self.is_portuguese() {
                        "Seleção manual".to_string()
                    } else {
                        "Manual selection".to_string()
                    }
                }
                PreferredLanguageSource::Route => {
                    if self.is_portuguese() {
                        "Idioma da URL".to_string()
                    } else {
                        "URL locale".to_string()
                    }
                }
                PreferredLanguageSource::Cookie => {
                    if self.is_portuguese() {
                        "Seleção salva".to_string()
                    } else {
                        "Saved selection".to_string()
                    }
                }
                PreferredLanguageSource::Browser => {
                    if self.is_portuguese() {
                        "Automático".to_string()
                    } else {
                        "Automatic".to_string()
                    }
                }
                PreferredLanguageSource::ArticleSource => {
                    if self.is_portuguese() {
                        "Edição original".to_string()
                    } else {
                        "Original edition".to_string()
                    }
                }
            }
        }
    }

    pub fn article_language_notice(
        self,
        preferred_language_name: &str,
        source_language_name: &str,
    ) -> String {
        if self.is_portuguese() {
            format!(
                "{} foi solicitado. Esta página está mostrando a edição original em {} enquanto a tradução é preparada.",
                preferred_language_name, source_language_name
            )
        } else {
            format!(
                "{} was requested. This page is currently showing the original {} edition while that translation is prepared.",
                preferred_language_name, source_language_name
            )
        }
    }

    pub fn comment_count_label(self, count: u64) -> String {
        if self.is_portuguese() {
            format!(
                "{} {}",
                count,
                if count == 1 {
                    "comentário"
                } else {
                    "comentários"
                }
            )
        } else {
            format!(
                "{} {}",
                count,
                if count == 1 { "comment" } else { "comments" }
            )
        }
    }

    pub fn comment_page_label(self, current_page: u64, total_pages: u64) -> String {
        if self.is_portuguese() {
            format!("Página {} / {}", current_page, total_pages)
        } else {
            format!("Page {} / {}", current_page, total_pages)
        }
    }

    pub fn translation_language_name(self, language: SupportedTranslationLanguage) -> &'static str {
        if !self.is_portuguese() {
            return language.name;
        }

        match language.code {
            "en" => "Inglês",
            "pt" => "Português",
            "es" => "Espanhol",
            "fr" => "Francês",
            "de" => "Alemão",
            "it" => "Italiano",
            _ => language.name,
        }
    }

    fn is_portuguese(self) -> bool {
        self.language.code == PORTUGUESE_LANGUAGE_CODE
    }
}

fn english_template_strings() -> Value {
    json!({
        "base": {
            "brand_note": "Daily bulletin of public affairs and avoidable concern",
            "create_article": "Draft article",
            "discord": "Discord",
            "admin": "Admin",
            "login": "Login",
            "logout": "Logout",
            "footer": "© The Wibble"
        },
        "index": {
            "front_page_eyebrow": "Front Page",
            "search_eyebrow": "Search Docket",
            "front_page_title": "Latest bulletins, notices, and administrative disturbances",
            "search_summary": "Review the current file of reports matching that term, then clear filters to return to the general state of concern.",
            "front_page_summary": "A straight-faced record of civic confusion, institutional overreaction, and matters requiring no immediate improvement.",
            "search_placeholder": "Search by department, incident, or headline",
            "search_button": "Search",
            "clear_button": "Clear",
            "toolbar_aria": "Feed controls",
            "toolbar_order": "Order",
            "toolbar_window": "Window",
            "lead_report": "Lead Report",
            "further_notices": "Further Notices",
            "additional_reports": "Additional reports from the desk",
            "load_more": "Load more reports",
            "empty_search_title": "No filed reports matched that search",
            "empty_search_body": "Try a broader term, return to newest, or clear the current docket and resume normal monitoring.",
            "empty_home_title": "No public bulletins have been issued yet",
            "empty_home_body": "The front page is presently calm. File a report and correct that condition.",
            "empty_home_cta": "Draft the first article",
            "reset_front_page": "Reset front page"
        },
        "create": {
            "eyebrow": "Prompt studio",
            "title": "Issue a matter for review",
            "summary": "Submit a scene, policy failure, administrative memo, or civic incident. The model works best when the premise is specific, official-sounding, and treated as entirely routine.",
            "logged_in_note": "Signed-in filings open as private drafts under your account. You can revise the copy, replace images, and publish when ready.",
            "anonymous_note": "Anonymous filings publish immediately if they succeed. Log in if you want an editable draft and explicit publish control.",
            "desk_quota": "Desk quota:",
            "owner_editing_note": "Owner editing is capped separately at %EDIT_HOURLY% edit-agent previews per hour.",
            "research_lane_note": "Research-backed filings run on their own desk at %RESEARCH_HOURLY% per hour / %RESEARCH_DAILY% per day.",
            "translation_lane_note": "Background translation refreshes stay on their own lane at %TRANSLATION_HOURLY% per hour.",
            "login_upsell": "Login raises the standard desk to %STANDARD_HOURLY% per hour, opens a bounded research desk at %RESEARCH_HOURLY% per hour, keeps results private as drafts, and unlocks the edit desk.",
            "tip_one": "Write it like a memo, not a punchline",
            "tip_two": "One absurd policy is enough",
            "tip_three": "Treat the impossible as procedure",
            "desk_mode": "Desk mode",
            "mode_auto": "Automatic",
            "mode_auto_note": "Use the cheap drafting path unless the brief clearly needs public-source grounding and you are signed in.",
            "mode_standard": "Standard",
            "mode_standard_note": "No browsing, fastest queue, best for invented institutions and generic bureaucratic collapse.",
            "mode_research": "Research desk",
            "mode_research_note": "Bounded public-source lookup for real institutions, policies, organizations, and public figures.",
            "mode_research_quota": "Separate quota: %RESEARCH_HOURLY% per hour / %RESEARCH_DAILY% per day.",
            "mode_research_login_prefix": "Log in",
            "mode_research_login_suffix": "to enable it.",
            "case_brief": "Case brief",
            "case_brief_placeholder": "Example: A metropolitan transit authority begins issuing formal emotional support updates alongside train delays, and commuters quickly start citing them in workplace absence forms.",
            "case_brief_help": "Name the institution, the failure mode, and the public reaction.",
            "draft_report": "Draft report",
            "wait_time": "Typical wait: a few seconds to a minute, depending on the image queue and the severity of the incident.",
            "result_private": "Result: private draft with the same background translation fallback rules as public stories.",
            "result_public": "Result: immediate public article. Login keeps the filing private until you approve it.",
            "sample_filings": "Sample filings",
            "sample_filings_body": "Tap a preset to load a straight-faced newsroom brief, then adjust the facts as required."
        },
        "wait": {
            "eyebrow": "Generation status",
            "working_on": "Working on:",
            "clarification_needed": "Clarification needed.",
            "loading": "Loading...",
            "progress_aria": "Generation progress",
            "images_planned": "Images planned",
            "completed": "Completed",
            "in_progress": "In progress",
            "failed": "Failed",
            "answer_label": "Answer the missing detail",
            "resume": "Resume drafting",
            "refresh": "Refresh now",
            "open_article": "Open article"
        },
        "error": {
            "image_alt": "Error image"
        },
        "content": {
            "images": "Images",
            "edit_article": "Edit article",
            "unpublished_warning": "This article is unpublished and hidden from the public feed and sitemap.",
            "filed_report": "Filed Report",
            "article_score": "Article score",
            "clear_upvote": "Clear upvote",
            "upvote": "Upvote",
            "login_to_upvote": "Log in to upvote",
            "clear_downvote": "Clear downvote",
            "downvote": "Downvote",
            "login_to_downvote": "Log in to downvote",
            "filed": "Filed",
            "status": "Status",
            "public": "Public",
            "draft": "Draft",
            "edition_desk": "Edition Desk",
            "current_edition": "Current edition",
            "edition_intro": "Automatic service follows your browser only when a prepared edition exists. If a requested edition is still being prepared, the source article stays readable immediately and the translation continues in the background. Manual changes are filed for this article only.",
            "public_response": "Public Response",
            "comments": "Comments",
            "add_comment": "Add a comment",
            "post_comment": "Post comment",
            "join_discussion_prefix": "Log in",
            "join_discussion_suffix": "to join the discussion.",
            "comments_closed": "Comments open on published articles.",
            "newer_comments": "Newer comments",
            "older_comments": "Older comments",
            "no_comments": "No comments yet."
        },
        "images": {
            "eyebrow": "Illustration archive",
            "title": "Generated Image Gallery",
            "summary": "Browse the visual side of Wibble stories and jump straight into the articles they belong to.",
            "used_in": "Used in",
            "next_page": "Next Page",
            "empty_title": "No gallery images yet",
            "empty_body": "Once articles finish rendering, their illustrations will show up here."
        },
        "image_info": {
            "kicker": "Image prompt",
            "metadata_aria": "Image metadata",
            "status": "Status",
            "created": "Created",
            "model": "Model",
            "last_error": "Last error",
            "used_in": "Used in"
        },
        "edit": {
            "eyebrow": "Editorial desk",
            "title": "Edit article",
            "jump_to_images": "Jump to images",
            "view_article": "View article",
            "workspace_note": "This workspace is available to the article owner and admins. Signed-in authors can revise draft copy, replace images, and publish when ready.",
            "agent_edit": "Agent edit",
            "agent_edit_body": "Describe the change in plain language. The agent will draft a revision, summarize it, and show a diff before anything is applied.",
            "change_request": "Change request",
            "change_request_placeholder": "Example: tighten the opening, cut the third section by a third, and make the closing sound more like an official bulletin.",
            "owners_only": "Owners and admins only. Nothing is applied until you approve the preview.",
            "max_chars": "Max %MAX_CHARS% characters.",
            "preview_revision": "Preview agent revision",
            "manual_editor": "Manual editor",
            "title_label": "Title",
            "description_label": "Description",
            "content_label": "Content (Markdown)",
            "save_changes": "Save changes",
            "cancel": "Cancel",
            "raw_markdown_note": "Raw markdown editing remains available as the escape hatch if the agent preview is not precise enough.",
            "images_title": "Images",
            "images_body": "Upload a replacement in JPG, JPEG, or PNG format, up to 12 MB. The file is validated before it overwrites the stored image.",
            "status_prefix": "Status:",
            "last_error_prefix": "Last error:",
            "accepted_formats": "Accepted formats: JPG, JPEG, PNG. Maximum size: 12 MB.",
            "replace_image": "Replace image",
            "regenerating": "Regenerating…",
            "regenerate_prompt": "Regenerate from prompt"
        },
        "edit_preview": {
            "eyebrow": "Editorial desk",
            "title": "Agent edit preview",
            "back_to_editor": "Back to editor",
            "view_article": "View article",
            "requested_change": "Requested change:",
            "agent_summary": "Agent summary:",
            "prompt_version": "Prompt version:",
            "field_preview": "Field preview",
            "current": "Current",
            "proposed": "Proposed",
            "diff": "Diff",
            "title_label": "Title",
            "description_label": "Description",
            "markdown_label": "Markdown",
            "apply_revision": "Apply agent revision",
            "discard_preview": "Discard preview"
        }
    })
}

fn portuguese_template_strings() -> Value {
    json!({
        "base": {
            "brand_note": "Boletim diário de assuntos públicos e preocupação evitável",
            "create_article": "Rascunhar artigo",
            "discord": "Discord",
            "admin": "Admin",
            "login": "Entrar",
            "logout": "Sair",
            "footer": "© The Wibble"
        },
        "index": {
            "front_page_eyebrow": "Capa",
            "search_eyebrow": "Arquivo de busca",
            "front_page_title": "Últimos boletins, comunicados e perturbações administrativas",
            "search_summary": "Revise o arquivo atual de relatórios que correspondem a esse termo e depois limpe os filtros para voltar ao estado geral de apreensão.",
            "front_page_summary": "Um registro sisudo de confusão cívica, exagero institucional e assuntos que não exigem melhora imediata.",
            "search_placeholder": "Busque por órgão, incidente ou manchete",
            "search_button": "Buscar",
            "clear_button": "Limpar",
            "toolbar_aria": "Controles do feed",
            "toolbar_order": "Ordem",
            "toolbar_window": "Período",
            "lead_report": "Relatório principal",
            "further_notices": "Outros avisos",
            "additional_reports": "Outros relatórios da redação",
            "load_more": "Carregar mais relatórios",
            "empty_search_title": "Nenhum relatório arquivado correspondeu a essa busca",
            "empty_search_body": "Tente um termo mais amplo, volte para os mais recentes ou limpe o arquivo atual e retome o monitoramento normal.",
            "empty_home_title": "Ainda não foram emitidos boletins públicos",
            "empty_home_body": "A capa está calma no momento. Registre um relatório e corrija essa situação.",
            "empty_home_cta": "Rascunhar o primeiro artigo",
            "reset_front_page": "Redefinir capa"
        },
        "create": {
            "eyebrow": "Estúdio de pautas",
            "title": "Registrar um assunto para análise",
            "summary": "Envie uma cena, falha de política, memorando administrativo ou incidente cívico. O modelo funciona melhor quando a premissa é específica, soa oficial e é tratada como totalmente rotineira.",
            "logged_in_note": "Envios com login abrem como rascunhos privados na sua conta. Você pode revisar o texto, trocar imagens e publicar quando estiver pronto.",
            "anonymous_note": "Envios anônimos são publicados imediatamente se derem certo. Entre se você quiser um rascunho editável e controle explícito de publicação.",
            "desk_quota": "Cota da redação:",
            "owner_editing_note": "A edição do autor tem limite separado de %EDIT_HOURLY% prévias do agente por hora.",
            "research_lane_note": "Pautas com pesquisa rodam em uma mesa própria, com %RESEARCH_HOURLY% por hora / %RESEARCH_DAILY% por dia.",
            "translation_lane_note": "As atualizações de tradução em segundo plano ficam em uma faixa separada de %TRANSLATION_HOURLY% por hora.",
            "login_upsell": "Entrar aumenta a mesa padrão para %STANDARD_HOURLY% por hora, abre uma mesa de pesquisa limitada com %RESEARCH_HOURLY% por hora, mantém os resultados privados como rascunhos e libera a mesa de edição.",
            "tip_one": "Escreva como memorando, não como piada",
            "tip_two": "Uma política absurda já basta",
            "tip_three": "Trate o impossível como procedimento",
            "desk_mode": "Modo da redação",
            "mode_auto": "Automático",
            "mode_auto_note": "Use o caminho barato de redação, a menos que o briefing claramente exija base em fontes públicas e você esteja logado.",
            "mode_standard": "Padrão",
            "mode_standard_note": "Sem navegação, fila mais rápida, melhor para instituições inventadas e colapsos burocráticos genéricos.",
            "mode_research": "Mesa de pesquisa",
            "mode_research_note": "Busca limitada em fontes públicas para instituições, políticas, organizações e figuras públicas reais.",
            "mode_research_quota": "Cota separada: %RESEARCH_HOURLY% por hora / %RESEARCH_DAILY% por dia.",
            "mode_research_login_prefix": "Entre",
            "mode_research_login_suffix": "para habilitar.",
            "case_brief": "Resumo do caso",
            "case_brief_placeholder": "Exemplo: uma autoridade metropolitana de transporte começa a publicar atualizações formais de apoio emocional junto com atrasos de trens, e passageiros passam a citá-las em justificativas de ausência no trabalho.",
            "case_brief_help": "Nomeie a instituição, a falha e a reação pública.",
            "draft_report": "Rascunhar relatório",
            "wait_time": "Espera típica: de alguns segundos a um minuto, dependendo da fila de imagens e da gravidade do incidente.",
            "result_private": "Resultado: rascunho privado com as mesmas regras de fallback de tradução em segundo plano das histórias públicas.",
            "result_public": "Resultado: artigo público imediato. Entrar mantém o envio privado até sua aprovação.",
            "sample_filings": "Exemplos de registros",
            "sample_filings_body": "Toque em um modelo para carregar um briefing sisudo de redação e ajuste os fatos conforme necessário."
        },
        "wait": {
            "eyebrow": "Status da geração",
            "working_on": "Trabalhando em:",
            "clarification_needed": "Esclarecimento necessário.",
            "loading": "Carregando...",
            "progress_aria": "Progresso da geração",
            "images_planned": "Imagens planejadas",
            "completed": "Concluídas",
            "in_progress": "Em andamento",
            "failed": "Falharam",
            "answer_label": "Responda o detalhe que falta",
            "resume": "Retomar rascunho",
            "refresh": "Atualizar agora",
            "open_article": "Abrir artigo"
        },
        "error": {
            "image_alt": "Imagem de erro"
        },
        "content": {
            "images": "Imagens",
            "edit_article": "Editar artigo",
            "unpublished_warning": "Este artigo não está publicado e fica oculto do feed público e do sitemap.",
            "filed_report": "Relatório protocolado",
            "article_score": "Pontuação do artigo",
            "clear_upvote": "Remover voto positivo",
            "upvote": "Voto positivo",
            "login_to_upvote": "Entre para votar positivo",
            "clear_downvote": "Remover voto negativo",
            "downvote": "Voto negativo",
            "login_to_downvote": "Entre para votar negativo",
            "filed": "Protocolado",
            "status": "Status",
            "public": "Público",
            "draft": "Rascunho",
            "edition_desk": "Mesa de edições",
            "current_edition": "Edição atual",
            "edition_intro": "O modo automático segue o navegador apenas quando já existe uma edição preparada. Se a edição solicitada ainda estiver sendo preparada, o artigo-fonte continua legível imediatamente e a tradução segue em segundo plano. Mudanças manuais valem apenas para este artigo.",
            "public_response": "Resposta pública",
            "comments": "Comentários",
            "add_comment": "Adicionar comentário",
            "post_comment": "Publicar comentário",
            "join_discussion_prefix": "Entre",
            "join_discussion_suffix": "para participar da discussão.",
            "comments_closed": "Comentários ficam abertos em artigos publicados.",
            "newer_comments": "Comentários mais novos",
            "older_comments": "Comentários mais antigos",
            "no_comments": "Ainda não há comentários."
        },
        "images": {
            "eyebrow": "Arquivo de ilustrações",
            "title": "Galeria de imagens geradas",
            "summary": "Veja o lado visual das histórias do Wibble e vá direto aos artigos a que elas pertencem.",
            "used_in": "Usada em",
            "next_page": "Próxima página",
            "empty_title": "Ainda não há imagens na galeria",
            "empty_body": "Quando os artigos terminarem de renderizar, as ilustrações aparecerão aqui."
        },
        "image_info": {
            "kicker": "Prompt da imagem",
            "metadata_aria": "Metadados da imagem",
            "status": "Status",
            "created": "Criada",
            "model": "Modelo",
            "last_error": "Último erro",
            "used_in": "Usada em"
        },
        "edit": {
            "eyebrow": "Mesa editorial",
            "title": "Editar artigo",
            "jump_to_images": "Ir para imagens",
            "view_article": "Ver artigo",
            "workspace_note": "Este espaço está disponível para o autor do artigo e para admins. Autores logados podem revisar o texto do rascunho, trocar imagens e publicar quando estiverem prontos.",
            "agent_edit": "Edição com agente",
            "agent_edit_body": "Descreva a mudança em linguagem simples. O agente vai redigir uma revisão, resumir o que mudou e mostrar um diff antes de qualquer aplicação.",
            "change_request": "Pedido de alteração",
            "change_request_placeholder": "Exemplo: deixe a abertura mais seca, corte o terceiro trecho em um terço e faça o encerramento soar mais como um boletim oficial.",
            "owners_only": "Só para autores e admins. Nada é aplicado até você aprovar a prévia.",
            "max_chars": "Máximo de %MAX_CHARS% caracteres.",
            "preview_revision": "Pré-visualizar revisão do agente",
            "manual_editor": "Editor manual",
            "title_label": "Título",
            "description_label": "Descrição",
            "content_label": "Conteúdo (Markdown)",
            "save_changes": "Salvar alterações",
            "cancel": "Cancelar",
            "raw_markdown_note": "A edição direta em markdown continua disponível como saída de emergência se a prévia do agente não ficar precisa o bastante.",
            "images_title": "Imagens",
            "images_body": "Envie uma substituição em JPG, JPEG ou PNG, com até 12 MB. O arquivo é validado antes de sobrescrever a imagem armazenada.",
            "status_prefix": "Status:",
            "last_error_prefix": "Último erro:",
            "accepted_formats": "Formatos aceitos: JPG, JPEG, PNG. Tamanho máximo: 12 MB.",
            "replace_image": "Substituir imagem",
            "regenerating": "Regenerando…",
            "regenerate_prompt": "Regenerar a partir do prompt"
        },
        "edit_preview": {
            "eyebrow": "Mesa editorial",
            "title": "Prévia da edição com agente",
            "back_to_editor": "Voltar ao editor",
            "view_article": "Ver artigo",
            "requested_change": "Alteração solicitada:",
            "agent_summary": "Resumo do agente:",
            "prompt_version": "Versão do prompt:",
            "field_preview": "Prévia dos campos",
            "current": "Atual",
            "proposed": "Proposto",
            "diff": "Diff",
            "title_label": "Título",
            "description_label": "Descrição",
            "markdown_label": "Markdown",
            "apply_revision": "Aplicar revisão do agente",
            "discard_preview": "Descartar prévia"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{default_site_language, resolve_site_language, site_text};
    use crate::llm::prompt_registry::find_supported_translation_language;

    #[test]
    fn unsupported_site_languages_fall_back_to_english() {
        let language = resolve_site_language(find_supported_translation_language("es"));

        assert_eq!(language.code, default_site_language().code);
    }

    #[test]
    fn portuguese_site_language_is_preserved() {
        let language = resolve_site_language(find_supported_translation_language("pt"));

        assert_eq!(language.code, "pt");
    }

    #[test]
    fn portuguese_template_strings_expose_localized_login() {
        let ui = site_text(find_supported_translation_language("pt").unwrap()).template_strings();

        assert_eq!(ui["base"]["login"].as_str(), Some("Entrar"));
    }
}
