use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sea_orm::prelude::*;
use sea_orm::{Database, EntityTrait, QueryOrder, ColumnTrait, Condition};
use tracing::{error, info, warn};

use wibble::entities::prelude::*;
use wibble::entities::{content, content_image, content_vote};
use sea_orm::TransactionTrait;

#[derive(Debug, Default)]
struct Counters {
    contents_deleted: u64,
    content_images_deleted: u64,
    content_votes_deleted: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    // Parse flags (simple parser, no extra deps)
    let mut dry_run = false;
    for arg in env::args().skip(1) {
        if arg == "--dry-run" || arg == "-n" { dry_run = true; }
        else if arg == "-h" || arg == "--help" { print_help(); return; }
        else { warn!("unknown_arg = {}", arg); }
    }

    // Validate IMAGES_DIR
    let images_dir = match env::var("IMAGES_DIR") {
        Ok(v) => v,
        Err(_) => {
            error!("IMAGES_DIR não está definida no ambiente");
            std::process::exit(2);
        }
    };
    let images_dir_path = PathBuf::from(&images_dir);
    if let Err(e) = fs::metadata(&images_dir_path) {
        error!("IMAGES_DIR inválida ou inacessível: {} - erro: {}", images_dir, e);
        std::process::exit(2);
    }

    // Connect DB
    let db_url = match env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            error!("DATABASE_URL não está definida no ambiente");
            std::process::exit(2);
        }
    };
    let db = match Database::connect(&db_url).await {
        Ok(db) => db,
        Err(e) => {
            error!("Falha ao conectar no banco: {}", e);
            std::process::exit(2);
        }
    };

    // Sanity ping (consulta barata)
    if let Err(e) = Content::find().count(&db).await {
        error!("Falha ao consultar o banco (ping): {}", e);
        std::process::exit(2);
    }

    info!("Conectado ao banco e IMAGES_DIR validada");

    // Carregar todos os conteúdos com as imagens relacionadas
    let contents_with_images = match Content::find()
        .order_by_asc(content::Column::CreatedAt)
        .find_with_related(ContentImage)
        .all(&db)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Erro carregando conteúdos: {}", e);
            std::process::exit(1);
        }
    };

    // Pré-contagem de votos por conteúdo (para estatísticas)
    let content_ids: Vec<String> = contents_with_images.iter().map(|(c, _)| c.id.clone()).collect();
    let mut votes_per_content: HashMap<String, u64> = HashMap::new();
    if !content_ids.is_empty() {
        // Em lotes para bases grandes
        const CHUNK: usize = 1000;
        for chunk in content_ids.chunks(CHUNK) {
            let cond = Condition::any().add(content_vote::Column::ContentId.is_in(chunk.to_vec()));
            let rows = match ContentVote::find().filter(cond).all(&db).await {
                Ok(v) => v,
                Err(e) => {
                    error!("Erro consultando content_vote: {}", e);
                    std::process::exit(1);
                }
            };
            for v in rows {
                *votes_per_content.entry(v.content_id).or_insert(0) += 1;
            }
        }
    }

    // Verificação de imagens existentes e identificação de órfãos
    let mut orphans: Vec<(content::Model, Vec<String>)> = Vec::new();
    for (c, images) in contents_with_images.iter() {
        let mut missing: Vec<String> = Vec::new();

        // Checa imagem principal (image_id) se houver
        if let Some(ref image_id) = c.image_id {
            if !file_exists(&images_dir_path, image_id) {
                missing.push(image_id.clone());
            }
        }
        // Checa todas as content_image
        for img in images.iter() {
            if !file_exists(&images_dir_path, &img.id) {
                missing.push(img.id.clone());
            }
        }
        if !missing.is_empty() {
            orphans.push((c.clone(), missing));
        }
    }

    // Output dos órfãos
    if orphans.is_empty() {
        println!("Nenhum artigo órfão encontrado.");
        return;
    }

    println!("Artigos órfãos encontrados ({}):", orphans.len());
    for (c, missing) in &orphans {
        println!("- id={} slug={} imagens_ausentes={:?}", c.id, c.slug, missing);
    }

    // Estatísticas planejadas
    let mut planned_images_to_delete: u64 = 0;
    let mut planned_votes_to_delete: u64 = 0;
    for (c, _missing) in &orphans {
        // Vamos apagar todas as imagens associadas ao conteúdo (mesmo que existam fisicamente)
        // pois o artigo será removido
        if let Ok(cnt) = ContentImage::find().filter(content_image::Column::ContentId.eq(c.id.clone())).count(&db).await {
            planned_images_to_delete += cnt;
        }
        planned_votes_to_delete += *votes_per_content.get(&c.id).unwrap_or(&0);
    }

    println!("Resumo do que será afetado:");
    println!("- content a remover: {}", orphans.len());
    println!("- content_image a remover: {}", planned_images_to_delete);
    println!("- content_vote a remover (por cascade): {}", planned_votes_to_delete);

    if dry_run {
        println!("--dry-run ativo: nenhuma alteração foi realizada.");
        return;
    }

    // Confirmação interativa
    println!("Deseja prosseguir com a exclusão? Digite 'yes' para confirmar:");
    print!("> ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        error!("Falha ao ler confirmação do usuário");
        std::process::exit(1);
    }
    if answer.trim() != "yes" {
        println!("Operação cancelada pelo usuário.");
        return;
    }

    // Execução das exclusões respeitando FKs: primeiro content_image (NoAction), depois content (Cascade em votes)
    let mut counters = Counters::default();

    for (c, _missing) in &orphans {
        let res = db
            .transaction(|tx| {
                let content_id = c.id.clone();
                Box::pin(async move {
                    // Deleta imagens relacionadas
                    let del_imgs = ContentImage::delete_many()
                        .filter(content_image::Column::ContentId.eq(content_id.clone()))
                        .exec(tx)
                        .await?;

                    // Contar votos antes de deletar content (serão apagados por cascade)
                    let votes_cnt = ContentVote::find()
                        .filter(content_vote::Column::ContentId.eq(content_id.clone()))
                        .count(tx)
                        .await? as u64;

                    // Deleta o conteúdo (cascade apaga votes)
                    let del_content = Content::delete_by_id(content_id.clone())
                        .exec(tx)
                        .await?;

                    Ok::<(u64, u64, u64), DbErr>((del_content.rows_affected, del_imgs.rows_affected, votes_cnt))
                })
            })
            .await;

        match res {
            Ok((c_del, imgs_del, votes_del)) => {
                counters.contents_deleted += c_del;
                counters.content_images_deleted += imgs_del;
                counters.content_votes_deleted += votes_del;
            }
            Err(e) => {
                error!("Erro ao deletar conteúdo id={}: {}", c.id, e);
            }
        }
    }

    println!("Operações concluídas.");
    println!("Resumo final:");
    println!("- content removidos: {}", counters.contents_deleted);
    println!("- content_image removidos: {}", counters.content_images_deleted);
    println!("- content_vote removidos (por cascade): {}", counters.content_votes_deleted);
}

fn file_exists(base: &Path, id: &str) -> bool {
    let p = base.join(format!("{}.jpg", id));
    fs::metadata(&p).is_ok()
}

fn print_help() {
    println!("Limpeza de artigos órfãos (imagens ausentes)\n");
    println!("Uso: clean_orphans [--dry-run]\n");
    println!("Variáveis de ambiente: ");
    println!("  DATABASE_URL  URL de conexão com PostgreSQL");
    println!("  IMAGES_DIR    Caminho do diretório das imagens (arquivos {{id}}.jpg)");
}
