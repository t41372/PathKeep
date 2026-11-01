use anyhow::{Context,Result};
use std::path::PathBuf;
use vault_core::{AppConfig,Vault,project_paths_with_root,save_config,run_backup,get_navigation_path};
use vault_store::Db;
fn main()->Result<()> {
    let root=PathBuf::from(std::env::args().nth(1).context("synthetic project root required")?);
    anyhow::ensure!(root.to_string_lossy().contains("pathkeep-rescue-native-v1-"),"only the generated fixture root is accepted");
    anyhow::ensure!(std::env::var_os("CHB_PROJECT_ROOT").as_ref().map(PathBuf::from).as_ref()==Some(&root),"all overrides must name this fixture");
    for key in ["CHB_CHROME_USER_DATA_DIR","CHB_FIREFOX_PROFILES_DIR","CHB_SAFARI_ROOT","CHB_TEST_KEYRING_DIR","PATHKEEP_PLATFORM_TEST_SANDBOX_DIR"] { anyhow::ensure!(std::env::var_os(key).is_some(),"missing {key}"); }
    let paths=project_paths_with_root(&root);
    std::fs::create_dir_all(&root)?;
    let config=AppConfig{initialized:true,git_enabled:false,capture_favicons:false,selected_profile_ids:vec!["chrome:Default".into(),"chrome:Profile 1".into(),"firefox:k3x9.default-release".into()],..AppConfig::default()};
    save_config(&paths,&config)?;
    let vault=Vault::open(paths,config,None)?;
    let report=run_backup(&vault,false)?;
    let ids=vault.store().read(Db::Archive,|s|Ok(s.query("SELECT v.id,u.url FROM visits v JOIN urls u ON u.id=v.url_id ORDER BY v.id",(),|r|Ok((r.get::<i64>(0)?,r.get::<String>(1)?)))?))?;
    let target=ids.iter().find(|(_,u)|u=="https://rescue-old.example/chain/49").context("chain target")?.0;
    let navigation=get_navigation_path(&vault,target)?;
    let result=serde_json::json!({"base":"ff89355071f04d1559be83e542f6ae3c4ee27df2","report":report,"visits":ids,"navigation":navigation});
    std::fs::write(root.join("expected-before-upgrade.json"),serde_json::to_vec_pretty(&result)?)?;
    vault.close()?;
    println!("{}",result);
    Ok(())
}
