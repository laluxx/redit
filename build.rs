use directories::ProjectDirs;
use std::fs::{self, create_dir_all};
use std::path::Path;

const ALWAYS_OVERWRITE_CONFIG: bool = true;

fn main() {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "Laluxx", "Redit") {
        let config_dir = proj_dirs.config_dir();
        let config_path = config_dir.join("config.lua");

        if !config_dir.exists() {
            create_dir_all(config_dir).expect("Failed to create configuration directory");
        }

        if ALWAYS_OVERWRITE_CONFIG || !config_path.exists() {
            let default_config_path = Path::new("config.lua");
            fs::copy(default_config_path, &config_path)
                .expect("Failed to copy default config file");
            println!("Config file copied to {:?}", config_path);
        }
    } else {
        panic!("Could not determine project directories");
    }
}