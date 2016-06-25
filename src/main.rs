/*
Copyright 2016 Tom Bebbington

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/
extern crate gtk;
extern crate thread_scoped;
extern crate cargo;
extern crate crates_io;

use std::path::Path;
use std::cell::RefCell;
use std::str;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::default::Default;
use std::sync::mpsc::{channel, Sender, Receiver};


use thread_scoped::scoped;

use cargo::ops::{self, VersionControl, ExecEngine, CompileOptions, CompileFilter, CompileMode, NewOptions};
use cargo::core::package::Package;
use cargo::util::config::Config;
use cargo::core::source::SourceId;

use crates_io::{Crate, Registry};

use gtk::prelude::*;
use gtk::{DIALOG_DESTROY_WITH_PARENT, DIALOG_MODAL, Notebook, Entry, ComboBoxText,SpinButton, Dialog, ButtonsType, Label, Builder, MessageType, MessageDialog, CellRendererText, FileChooser, TreeView, Button, ListStore, TreeViewColumn, Window, SearchEntry};

fn make_column(title: &str, kind: &str, id: i32) -> TreeViewColumn {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.set_title(title);
    column.pack_start(&cell, true);
    column.add_attribute(&cell, kind, id);
    column.set_sort_column_id(id);
    column
}

fn dialog(parent: Option<&Window>, ty: MessageType, text: &str) {
    let dialog = MessageDialog::new(parent, DIALOG_DESTROY_WITH_PARENT | DIALOG_MODAL, ty, ButtonsType::Close, text);
    dialog.connect_response(|dialog, _| {
        dialog.destroy();
    });
    dialog.set_icon_name(Some("dialog_error"));
    dialog.set_deletable(true);
    dialog.show_all();
    dialog.present();
    dialog.run();
}

fn error(parent: Option<&Window>, text: &str) {
    dialog(parent, MessageType::Error, text)
}
fn info(parent: Option<&Window>, text: &str) {
    dialog(parent, MessageType::Info, text)
}

fn bind_file_button<F>(file: &FileChooser, button: &Button, f: F) where F: Fn(&Path) + 'static {
    let file = file.clone();
    button.connect_clicked(move |_| {
        if let Some(file) = file.get_filename() {
            f(&file);
        }
    });
}

macro_rules! error {
    ($window:expr, $($tt:tt)*) => (error($window, &format!($($tt)*)))
}
macro_rules! info {
    ($window:expr, $($tt:tt)*) => (info($window, &format!($($tt)*)))
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

#[derive(Clone)]
pub struct OptionsContext {
    pub options: Rc<RefCell<Options>>,
    pub dialog: Dialog,
    pub jobs: SpinButton,
    pub ok: Button,
    pub cancel: Button
}
impl OptionsContext {
    fn new(options: Rc<RefCell<Options>>) -> OptionsContext {
        let builder = Builder::new_from_string(include_str!("compile.glade"));
        let this = OptionsContext {
            options: options,
            dialog: builder.get_object("dialog").unwrap(),
            jobs: builder.get_object("jobs").unwrap(),
            ok: builder.get_object("ok").unwrap(),
            cancel: builder.get_object("cancel").unwrap(),
        };
        this.bind_listeners();
        this.dialog.show_all();
        this
    }
    fn save(&self) {
        let mut options = self.options.borrow_mut();
        options.jobs = Some(self.jobs.get_value_as_int() as u32);
    }
    fn bind_listeners(&self) {
        let self2 = self.clone();
        self.ok.connect_clicked(move |_| {
            self2.save();
            self2.dialog.destroy();
        });
        let self3 = self.clone();
        self.cancel.connect_clicked(move |_| {
            self3.dialog.destroy();
        });
    }
}

pub trait Page: Clone {
    fn new(context: &PreContext) -> Self;
    fn update(&self) {}
    fn bind_listeners(&self) {}
}

#[derive(Clone)]
pub struct OnlineSearchPage {
    pub config: Rc<Config>,
    pub options: Rc<RefCell<Options>>,
    pub window: Window,
    pub search: SearchEntry,
    pub store: ListStore,
    pub packages: TreeView,
    pub install: Button,
    pub sender: Sender<Vec<Crate>>,
    pub receiver: Rc<Receiver<Vec<Crate>>>,
    results: Rc<RefCell<Vec<Crate>>>
}
impl Page for OnlineSearchPage {
    fn new(c: &PreContext) -> Self {
        let (tx, rx) = channel();
        OnlineSearchPage {
            config: c.config.clone(),
            options: c.options.clone(),
            window: c.window.clone(),
            search: c.builder.get_object("online_search").unwrap(),
            packages: c.builder.get_object("online_packages").unwrap(),
            install: c.builder.get_object("install").unwrap(),
            store: c.builder.get_object("store").unwrap(),
            sender: tx,
            receiver: Rc::new(rx),
            results: Rc::new(RefCell::new(Vec::new()))
        }
    }
    fn update(&self) {
        if let Ok(crates) = self.receiver.try_recv() {
            for old_result in self.packages.get_children() {
                self.packages.remove(&old_result);
            }
            let mut results = self.results.borrow_mut();
            *results = crates;
            self.store.clear();
            for c in &*results {
                self.store.insert_with_values(None, &[0, 1, 2], &[&c.name, &c.description, &c.max_version]);
            }
            self.packages.show_all();
        }
    }
    fn bind_listeners(&self) {
        let results = self.results.clone();
        let window = self.window.clone();
        let config = self.config.clone();
        let options = self.options.clone();
        let packages = self.packages.clone();
        self.install.connect_clicked(move |_| {
            let index = packages.get_cursor().0.unwrap().get_indices()[0];
            let results = results.borrow();
            let id = SourceId::for_central(&*config).unwrap();
            let options = options.borrow();
            let options: CompileOptions = (&*options).into();
            let name = &results[index as usize].name;
            if let Err(err) = ops::install(None, Some(name), &id, None, &options) {
                error!(Some(&window), "Failed to install '{}': {:?}", name, err);
            } else {
                info!(Some(&window), "Crate '{}' successfully installed", name);
            }
        });
        let thread = RefCell::new(None);
        let sender = self.sender.clone();
        self.search.connect_activate(move |entry| {
            let query = entry.get_text().unwrap_or_else(|| "".to_string());
            let tx = Arc::new(Mutex::new(sender.clone()));
            let mut thread = thread.borrow_mut();
            *thread = Some(unsafe { scoped(move || {
                let mut registry = get_registry();
                let tx = tx.lock().unwrap();
                tx.send(registry.search(&query, 64).map_err(|_| "Search failed").unwrap().0).unwrap();
            })});
        });
    }
}

#[derive(Clone)]
pub struct LocalPackagePage {
    pub options: Rc<RefCell<Options>>,
    pub config: Rc<Config>,
    pub window: Window,
    pub file: FileChooser,
    pub build: Button,
    pub run: Button,
    pub bench: Button,
    pub publish: Button,
    pub update: Button,
    pub test: Button,
    pub doc: Button,
    pub name: Label,
    pub version: Label,
    pub author: Label,
    pub description: Label,
    pub configure_compile: Button
}
impl LocalPackagePage {
    fn bind_compile_button(&self, name: &'static str, button: &Button, mode: CompileMode) {
        let window = self.window.clone();
        let options = self.options.clone();
        bind_file_button(&self.file, button, move |file| {
            let options = options.borrow();
            let mut options: CompileOptions = (&*options).into();
            options.mode = mode;
            if let Err(err) = ops::compile(&file, &options) {
                error!(Some(&window), "Failed to run '{}' subcommand due to '{:?}': {:?}", name, file, err);
            } else {
                info!(Some(&window), "Successfully ran subcommand '{}'", name);
            }
        });
    }
}
impl Page for LocalPackagePage {
    fn new(c: &PreContext) -> Self {
        let b = &c.builder;
        LocalPackagePage {
            options: c.options.clone(),
            config: c.config.clone(),
            window: c.window.clone(),
            file: b.get_object("file").unwrap(),
            build: b.get_object("build").unwrap(),
            run: b.get_object("run").unwrap(),
            bench: b.get_object("bench").unwrap(),
            publish: b.get_object("publish").unwrap(),
            update: b.get_object("update").unwrap(),
            test: b.get_object("test").unwrap(),
            doc: b.get_object("doc").unwrap(),
            configure_compile: b.get_object("configure-compile").unwrap(),
            name: b.get_object("local_name").unwrap(),
            version: b.get_object("local_version").unwrap(),
            author: b.get_object("local_author").unwrap(),
            description: b.get_object("local_description").unwrap(),
        }
    }
    fn update(&self) {
        if let Some(name) = self.file.get_filename() {
            match Package::for_path(&name, &*self.config){
                Ok(pack) => {
                    let meta = pack.manifest().metadata();
                    update_labels(&[
                        (&self.name, pack.name()),
                        (&self.description, meta.description.as_ref().map(|v| v as &str).unwrap_or("")),
                        (&self.author, meta.authors.get(0).as_ref().map(|v| v as &str).unwrap_or("")),
                        (&self.version, &pack.version().to_string())
                    ]);
                },
                Err(error) =>
                    error!(Some(&self.window), "Failed to parse Cargo.toml: {:?}", error)
            }
        }
    }
    fn bind_listeners(&self) {
        self.bind_compile_button("build", &self.build, CompileMode::Build);
        self.bind_compile_button("test", &self.test, CompileMode::Test);
        self.bind_compile_button("bench", &self.bench, CompileMode::Bench);
        self.bind_compile_button("doc", &self.doc, CompileMode::Doc { deps: true });
        let options = self.options.clone();
        self.configure_compile.connect_clicked(move |_| {
            OptionsContext::new(options.clone());
        });
    }
}
#[derive(Clone)]
pub struct NewPackagePage {
    pub window: Window,
    pub config: Rc<Config>,
    pub file: FileChooser,
    pub new: Button,
    pub name: Entry,
    pub ty: ComboBoxText,
    pub vcs: ComboBoxText,
}
impl Page for NewPackagePage {
    fn new(c: &PreContext) -> NewPackagePage {
        let b = &c.builder;
        NewPackagePage {
            window: c.window.clone(),
            config: c.config.clone(),
            file: b.get_object("package-file").unwrap(),
            name: b.get_object("package-name").unwrap(),
            vcs: b.get_object("package-vcs").unwrap(),
            ty: b.get_object("package-type").unwrap(),
            new: b.get_object("package-new").unwrap()
        }
    }
    fn bind_listeners(&self) {
        let name = self.name.clone();
        let ty = self.ty.clone();
        let vcs = self.vcs.clone();
        let window = self.window.clone();
        let config = self.config.clone();
        bind_file_button(&self.file, &self.new, move |file| {
            let text = name.get_text();
            let opts = NewOptions {
                path: file.to_str().unwrap(),
                name: text.as_ref().map(String::as_str),
                bin: ty.get_active_id().as_ref().map(String::as_str) == Some("bin"),
                version_control: vcs.get_active_id().as_ref().map(String::as_str).map(|v| match v {
                    "git" => VersionControl::Git,
                    "mercurial" => VersionControl::Hg,
                    _ => VersionControl::NoVcs
                })
            };
            if ops::init(opts, &config).is_ok() {
                info(Some(&window), "Created crate successfully");
            } else {
                error(Some(&window), "Failed to create crate");
            }
        });
    }
}

#[derive(Clone)]
pub struct Context {
    pub config: Rc<Config>,
    pub builder: Builder,
    pub window: Window,
    pub tabs: Notebook,
    pub online_packs: OnlineSearchPage,
    pub local_pack: LocalPackagePage,
    pub new_pack: NewPackagePage,
    pub options: Rc<RefCell<Options>>
}

#[derive(Clone)]
pub struct PreContext {
    pub config: Rc<Config>,
    pub builder: Builder,
    pub window: Window,
    pub options: Rc<RefCell<Options>>
}

fn update_labels(labels: &[(&Label, &str)]) {
    for &(label, text) in labels {
        label.set_text(text);
    }
}

fn get_registry() -> Registry {
    Registry::new("https://crates.io".into(), None)
}

impl Context {
    fn new() -> Context {
        let builder = Builder::new_from_string(include_str!("layout.glade"));
        let packs: TreeView = builder.get_object("online_packages").expect("Failed to load online_packages");
        let store: ListStore = builder.get_object("store").unwrap();
        packs.set_headers_visible(true);
        packs.append_column(&make_column("Package", "text", 0));
        packs.append_column(&make_column("Description", "text", 1));
        packs.append_column(&make_column("Version", "text", 2));
        packs.set_model(Some(&store));
        let tabs = builder.get_object("tabs").unwrap();
        let pre = PreContext {
            window: builder.get_object("window").unwrap(),
            builder: builder,
            config: Rc::new(Config::default().unwrap()),
            options: Rc::new(RefCell::new(Options::default()))
        };
        let local_pack = LocalPackagePage::new(&pre);
        let online_packs = OnlineSearchPage::new(&pre);
        let new_pack = NewPackagePage::new(&pre);
        Context {
            window: pre.window,
            builder: pre.builder,
            config: pre.config,
            options: pre.options,
            tabs: tabs,
            local_pack: local_pack,
            online_packs: online_packs,
            new_pack: new_pack
        }
    }
    fn bind_listeners(&self) {
        self.online_packs.bind_listeners();
        self.local_pack.bind_listeners();
        self.new_pack.bind_listeners();
        let online_packs = self.online_packs.clone();
        let local_pack = self.local_pack.clone();
        let new_pack = self.new_pack.clone();
        self.window.connect_draw(move |_, _| {
            online_packs.update();
            local_pack.update();
            new_pack.update();
            Inhibit(false)
        });
        self.window.connect_delete_event(|_, _| {
            gtk::main_quit();
            Inhibit(false)
        });
    }
}

fn main() {
    gtk::init().unwrap_or_else(|_| panic!("{}", "cargo-manager: failed to initialize GTK."));
    let c = Context::new();
    c.bind_listeners();
    c.window.show_all();
    gtk::main();
}