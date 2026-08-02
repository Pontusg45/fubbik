use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Fubbik API", version = "0.1.0"),
    paths(
        crate::chunks::routes::list_chunks,
        crate::chunks::routes::create_chunk,
        crate::chunks::routes::get_chunk,
        crate::chunks::routes::update_chunk,
        crate::chunks::routes::delete_chunk,
        crate::chunks::routes::chunk_history,
        crate::chunks::routes::get_applies_to,
        crate::chunks::routes::put_applies_to,
        crate::chunks::routes::get_file_refs,
        crate::chunks::routes::put_file_refs,
    ),
    components(schemas(
        fubbik_db::repo::chunk::Chunk,
        fubbik_db::repo::chunk::Sort,
        fubbik_db::repo::chunk_version::ChunkVersion,
        fubbik_db::repo::chunk_meta::AppliesTo,
        fubbik_db::repo::chunk_meta::FileRef,
        crate::chunks::dto::CreateChunkBody,
        crate::chunks::dto::UpdateChunkBody,
        crate::chunks::dto::ChunkListResponse,
        crate::chunks::routes::PatternsBody,
        crate::chunks::routes::PathsBody,
    ))
)]
pub struct ApiDoc;
