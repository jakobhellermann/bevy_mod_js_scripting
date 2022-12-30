use std::path::PathBuf;

use bevy::{
    asset::{AssetLoader, BoxedFuture, LoadedAsset},
    prelude::Resource,
};
use bevy_reflect::TypeUuid;

#[derive(TypeUuid, Resource)]
#[uuid = "34186503-91f4-4afa-99fc-c0c3688a9439"]
pub struct JsScript {
    pub source: String,
    pub path: PathBuf,
}

pub struct JsScriptLoader;
impl AssetLoader for JsScriptLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> BoxedFuture<'a, anyhow::Result<(), anyhow::Error>> {
        Box::pin(async move {
            let source = String::from_utf8(bytes.to_vec())?;

            let source = crate::transpile::transpile(load_context.path(), &source)?;

            load_context.set_default_asset(LoadedAsset::new(JsScript {
                source,
                path: load_context.path().to_path_buf(),
            }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["js", "ts"]
    }
}
