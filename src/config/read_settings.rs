use std::env;
use std::default::Default;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quire::parse_config;
use quire::validate as V;

use config::Settings;
use path_util::{PathExt, Expand};


#[derive(PartialEq, RustcDecodable, Debug)]
struct SecureSettings {
    allowed_dirs: BTreeMap<String, PathBuf>,
    storage_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    version_check: Option<bool>,
    proxy_env_vars: Option<bool>,
    ubuntu_mirror: Option<String>,
    alpine_mirror: Option<String>,
    site_settings: BTreeMap<PathBuf, SecureSettings>,
}

pub fn secure_settings_validator<'a>(has_children: bool)
    -> Box<V::Validator + 'a>
{
    let mut members = vec!(
        ("allowed_dirs".to_string(), Box::new(V::Mapping {
            key_element: Box::new(V::Scalar {
                .. Default::default()}) as Box<V::Validator>,
            value_element: Box::new(V::Scalar {
                .. Default::default()}) as Box<V::Validator>,
            .. Default::default()}) as Box<V::Validator>),
        ("storage_dir".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("cache_dir".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("version_check".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("proxy_env_vars".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("ubuntu_mirror".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("alpine_mirror".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
    );
    if has_children {
        members.push(
            ("site_settings".to_string(), Box::new(V::Mapping {
                key_element: Box::new(V::Scalar {
                    .. Default::default()}) as Box<V::Validator>,
                value_element: secure_settings_validator(false),
                .. Default::default()}) as Box<V::Validator>),
        );
    }
    return Box::new(V::Structure {
        members: members, .. Default::default()}) as Box<V::Validator>;
}

#[derive(PartialEq, RustcDecodable)]
struct InsecureSettings {
    version_check: Option<bool>,
    shared_cache: Option<bool>,
    ubuntu_mirror: Option<String>,
    alpine_mirror: Option<String>,
}

pub fn insecure_settings_validator<'a>() -> Box<V::Validator + 'a> {
    return Box::new(V::Structure { members: vec!(
        ("version_check".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator + 'a>),
        ("shared_cache".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator + 'a>),
        ("ubuntu_mirror".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
        ("alpine_mirror".to_string(), Box::new(V::Scalar {
            optional: true,
            .. Default::default()}) as Box<V::Validator>),
    ), .. Default::default()}) as Box<V::Validator>;
}

#[derive(Debug)]
pub struct MergedSettings {
    pub allowed_dirs: BTreeMap<String, PathBuf>,
    pub allowed_files: BTreeMap<String, PathBuf>,
    pub storage_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub shared_cache: bool,
}

pub fn read_settings(project_root: &Path)
    -> Result<(MergedSettings, Settings), String>
{
    let mut ext_settings = MergedSettings {
        allowed_dirs: BTreeMap::new(),
        allowed_files: BTreeMap::new(),
        storage_dir: None,
        cache_dir: None,
        shared_cache: false,
    };
    let mut int_settings = Settings {
        proxy_env_vars: true,
        version_check: true,
        uid_map: None,
        ubuntu_mirror: "mirror://mirrors.ubuntu.com/mirrors.txt".to_string(),
        alpine_mirror: None,
    };
    let mut secure_files = vec!();
    if let Ok(home) = env::var("VAGGA_USER_HOME") {
        let home = Path::new(&home);
        secure_files.push(home.join(".config/vagga/settings.yaml"));
        secure_files.push(home.join(".vagga/settings.yaml"));
        secure_files.push(home.join(".vagga.yaml"));
    };
    for filename in secure_files.iter() {
        if !filename.exists() {
            continue;
        }
        let cfg: SecureSettings = try!(parse_config(filename,
            &*secure_settings_validator(true), Default::default()));
        for (k, v) in cfg.allowed_dirs.iter() {
            ext_settings.allowed_dirs.insert(k.clone(), v.clone());
        }
        if let Some(dir) = cfg.storage_dir {
            ext_settings.storage_dir = Some(try!(dir.expand_home()
                .map_err(|()| format!("Can't expand tilde `~` in storage dir \
                    no HOME found"))));
        }
        if let Some(dir) = cfg.cache_dir {
            ext_settings.cache_dir = Some(try!(dir.expand_home()
                .map_err(|()| format!("Can't expand tilde `~` in cache dir \
                    no HOME found"))));
            ext_settings.shared_cache = true;
        }
        if let Some(val) = cfg.version_check {
            int_settings.version_check = val;
        }
        if let Some(val) = cfg.proxy_env_vars {
            int_settings.proxy_env_vars = val;
        }
        if let Some(ref val) = cfg.ubuntu_mirror {
            int_settings.ubuntu_mirror = val.clone();
        }
        if let Some(ref val) = cfg.alpine_mirror {
            int_settings.alpine_mirror = Some(val.clone());
        }
        if let Some(cfg) = cfg.site_settings.get(project_root) {
            for (k, v) in cfg.allowed_dirs.iter() {
                ext_settings.allowed_dirs.insert(k.clone(), v.clone());
            }
            if let Some(ref dir) = cfg.storage_dir {
                ext_settings.storage_dir = Some(dir.clone());
            }
            if let Some(ref dir) = cfg.cache_dir {
                ext_settings.cache_dir = Some(dir.clone());
                ext_settings.shared_cache = true;
            }
            if let Some(val) = cfg.version_check {
                int_settings.version_check = val;
            }
            if let Some(ref val) = cfg.ubuntu_mirror {
                int_settings.ubuntu_mirror = val.clone();
            }
            if let Some(ref val) = cfg.alpine_mirror {
                int_settings.alpine_mirror = Some(val.clone());
            }
        }
    }
    let mut insecure_files = vec!();
    insecure_files.push(project_root.join(".vagga.settings.yaml"));
    insecure_files.push(project_root.join(".vagga/settings.yaml"));
    for filename in insecure_files.iter() {
        if !filename.exists() {
            continue;
        }
        let cfg: InsecureSettings = try!(parse_config(filename,
            &*insecure_settings_validator(), Default::default()));
        if let Some(val) = cfg.version_check {
            int_settings.version_check = val;
        }
        if let Some(ref val) = cfg.ubuntu_mirror {
            int_settings.ubuntu_mirror = val.clone();
        }
        if let Some(ref val) = cfg.alpine_mirror {
            int_settings.alpine_mirror = Some(val.clone());
        }
        if let Some(val) = cfg.shared_cache {
            ext_settings.shared_cache = val;
        }
    }
    return Ok((ext_settings, int_settings));
}
