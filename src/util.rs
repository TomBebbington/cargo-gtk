use std::cell::RefCell;
use std::sync::Arc;
use std::rc::Rc;

use cargo::util::config::Config;
use cargo::ops::{ExecEngine, CompileOptions, CompileFilter, CompileMode};

use gtk::{Window, Builder};

#[derive(Clone)]
pub struct PreContext {
    pub config: Rc<Config>,
    pub builder: Builder,
    pub window: Window,
    pub options: Rc<RefCell<Options>>
}

#[derive(Clone)]
pub enum Filter {
    Everything
}
impl<'a> Into<CompileFilter<'a>> for &'a Filter {
    fn into(self) -> CompileFilter<'a> {
        match self {
            &Filter::Everything => CompileFilter::Everything
        }
    }
}

pub struct Options {
    pub config: Config,
    pub jobs: Option<u32>,
    pub target: Option<String>,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub spec: Vec<String>,
    pub filter: Filter,
    pub exec_engine: Option<Arc<Box<ExecEngine>>>,
    pub release: bool,
    pub mode: CompileMode,
    pub target_rustdoc_args: Option<Vec<String>>,
    pub target_rustc_args: Option<Vec<String>>
}
impl Default for Options {
    fn default() -> Options {
        Options {
            jobs: Some(8),
            features: Vec::new(),
            spec: Vec::new(),
            config: Config::default().unwrap(),
            exec_engine: None,
            release: false,
            filter: Filter::Everything,
            no_default_features: false,
            target: None,
            target_rustdoc_args: None,
            target_rustc_args: None,
            mode: CompileMode::Build
        }
    }
}
impl<'a> Into<CompileOptions<'a>> for &'a Options {
    fn into(self) -> CompileOptions<'a> {
        CompileOptions {
            config: &self.config,
            jobs: self.jobs,
            target: self.target.as_ref().map(|v| v as &str),
            features: &self.features,
            no_default_features: self.no_default_features,
            spec: &self.spec,
            filter: (&self.filter).into(),
            exec_engine: self.exec_engine.clone(),
            release: self.release,
            mode: self.mode,
            target_rustdoc_args: self.target_rustdoc_args.as_ref().map(|v| v as &[String]),
            target_rustc_args: self.target_rustc_args.as_ref().map(|v| v as &[String])
        }
    }
}
