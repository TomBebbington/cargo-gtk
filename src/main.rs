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
use std::process::Command;
use std::default::Default;
use std::sync::mpsc::channel;


use thread_scoped::scoped;

use cargo::ops::{self, VersionControl, ExecEngine, CompileOptions, CompileFilter, CompileMode, NewOptions};
use cargo::core::package::Package;
use cargo::util::config::Config;
use cargo::core::source::SourceId;

use crates_io::{Crate, Registry};

use gtk::prelude::*;
use gtk::{DIALOG_DESTROY_WITH_PARENT, DIALOG_MODAL, Entry, ComboBoxText,SpinButton, Dialog, ButtonsType, Label, Builder, MessageType, MessageDialog, CellRendererText, FileChooser, TreeView, Button, ListStore, TreeViewColumn, Window, SearchEntry};

fn make_column(title: &str, kind: &str, id: i32) -> TreeViewColumn {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.set_title(title);
    column.pack_start(&cell, true);
    column.add_attribute(&cell, kind, id);
    column.set_sort_column_id(id);
    column
}
fn show_console(cwd: &Path, cmdn: &str) {
    let mut cmd = Command::new("gnome-terminal");
    cmd.current_dir(cwd);
    cmd.arg("-x");
    cmd.arg("bash");
    cmd.arg("-c");
    cmd.arg(cmdn);
    println!("{:?}", cmd);
    cmd.spawn().unwrap();
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

#[derive(Clone)]
pub struct Context {
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
    pub store: ListStore,
    pub install: Button,
    pub local_name: Label,
    pub local_version: Label,
    pub local_author: Label,
    pub local_description: Label,
    pub online_search: SearchEntry,
    pub online_packages: TreeView,
    pub configure_compile: Button,
    pub package_file: FileChooser,
    pub package_new: Button,
    pub package_name: Entry,
    pub package_type: ComboBoxText,
    pub package_vcs: ComboBoxText,
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
        packs.set_headers_visible(true);
        packs.append_column(&make_column("Package", "text", 0));
        packs.append_column(&make_column("Description", "text", 1));
        packs.append_column(&make_column("Version", "text", 2));
        Context {
            config: Rc::new(Config::default().unwrap()),
            options: Rc::new(RefCell::new(Options::default())),
            window: builder.get_object("window").unwrap(),
            file: builder.get_object("file").unwrap(),
            build: builder.get_object("build").unwrap(),
            run: builder.get_object("run").unwrap(),
            bench: builder.get_object("bench").unwrap(),
            publish: builder.get_object("publish").unwrap(),
            update: builder.get_object("update").unwrap(),
            store: builder.get_object("store").unwrap(),
            test: builder.get_object("test").unwrap(),
            doc: builder.get_object("doc").unwrap(),
            install: builder.get_object("install").unwrap(),
            configure_compile: builder.get_object("configure-compile").unwrap(),
            local_name: builder.get_object("local_name").unwrap(),
            local_version: builder.get_object("local_version").unwrap(),
            local_author: builder.get_object("local_author").unwrap(),
            local_description: builder.get_object("local_description").unwrap(),
            online_search: builder.get_object("online_search").unwrap(),
            online_packages: packs,
            package_file: builder.get_object("package-file").unwrap(),
            package_name: builder.get_object("package-name").unwrap(),
            package_vcs: builder.get_object("package-vcs").unwrap(),
            package_type: builder.get_object("package-type").unwrap(),
            package_new: builder.get_object("package-new").unwrap()
        }
    }
    fn bind_compile_button<F>(&self, name: &'static str, button: &Button, mods: F) where F: Fn(&mut CompileOptions) + 'static {
        let file = self.file.clone();
        let window = self.window.clone();
        let options = self.options.clone();
        button.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                let options = options.borrow();
                let mut options: CompileOptions = (&*options).into();
                mods(&mut options);
                if let Err(err) = ops::compile(&file, &options) {
                    error!(Some(&window), "Failed to run '{}' subcommand due to '{:?}': {:?}", name, file, err);
                } else {
                    info!(Some(&window), "Successfully ran subcommand '{}'", name);
                }
            }
        });
    }
    fn update(&self) {
        if let Some(name) = self.file.get_filename() {
            match Package::for_path(&name, &*self.config){
                Ok(pack) => {
                    let meta = pack.manifest().metadata();
                    update_labels(&[
                        (&self.local_name, pack.name()),
                        (&self.local_description, meta.description.as_ref().map(|v| v as &str).unwrap_or("")),
                        (&self.local_author, meta.authors.get(0).as_ref().map(|v| v as &str).unwrap_or("")),
                        (&self.local_version, &pack.version().to_string())
                    ]);
                },
                Err(error) =>
                    error!(Some(&self.window), "Failed to parse Cargo.toml: {:?}", error)
            }
        }
    }
    fn bind_listeners(&self) {
        self.online_packages.set_model(Some(&self.store));
        let packs = self.online_packages.clone();
        let results = Rc::new(RefCell::new(Vec::<Crate>::new()));
        let results2 = results.clone();
        let packs2 = self.online_packages.clone();
        let (tx, rx) = channel();
        let thread = RefCell::new(None);


        self.online_search.connect_activate(move |entry| {
            let query = entry.get_text().unwrap_or_else(|| "".to_string());
            let tx = Arc::new(Mutex::new(tx.clone()));
            let mut thread = thread.borrow_mut();
            *thread = Some(unsafe { scoped(move || {
                let mut registry = get_registry();
                let tx = tx.lock().unwrap();
                tx.send(registry.search(&query, 64).map_err(|_| "Search failed").unwrap().0).unwrap();
            })});
        });
        self.bind_compile_button("build", &self.build, |_| {});
        self.bind_compile_button("test", &self.test, |mut c| c.mode = CompileMode::Test);
        self.bind_compile_button("bench", &self.bench, |mut c| c.mode = CompileMode::Bench);
        let options = self.options.clone();
        self.configure_compile.connect_clicked(move |_| {
            let _ = OptionsContext::new(options.clone());
        });
        let window = self.window.clone();
        let config = self.config.clone();
        let options = self.options.clone();
        self.install.connect_clicked(move |_| {
            let index = packs2.get_cursor().0.unwrap().get_indices()[0];
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
        let window = self.window.clone();
        let file = self.file.clone();
        let options = self.options.clone();
        self.run.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                let options = options.borrow();
                let options: CompileOptions = (&*options).into();
                if let Err(err) = ops::run(&file, &options, &[]) {
                    error!(Some(&window), "Failed to run '{:?}': {:?}", file, err);
                }
            }
        });
        let window = self.window.clone();
        let file = self.file.clone();
        let config = self.config.clone();
        self.publish.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                let package = Package::for_path(&file, &*config).unwrap();
                if package.publish() {
                    info(Some(&window), "Crate successfully published")
                } else {
                    error(Some(&window), "Failed to publish crate");
                }
            }
        });
        let file = self.package_file.clone();
        let package_name = self.package_name.clone();
        let package_type = self.package_type.clone();
        let package_vcs = self.package_vcs.clone();
        let window = self.window.clone();
        let options = self.options.clone();
        self.package_new.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                let text = package_name.get_text();
                let opts = NewOptions {
                    path: file.to_str().unwrap(),
                    name: text.as_ref().map(String::as_str),
                    bin: package_type.get_active_id().as_ref().map(String::as_str) == Some("bin"),
                    version_control: package_vcs.get_active_id().as_ref().map(String::as_str).map(|v| match v {
                        "git" => VersionControl::Git,
                        "mercurial" => VersionControl::Hg,
                        _ => VersionControl::NoVcs
                    })
                };
                let options = options.borrow();
                if ops::init(opts, &options.config).is_ok() {
                    info(Some(&window), "Created crate successfully");
                } else {
                    error(Some(&window), "Failed to create crate");
                }
            }
        });
        let self2 = self.clone();
        let old_file = RefCell::new(self.file.get_filename());
        let file = self.file.clone();
        let store = self.store.clone();
        self.window.connect_draw(move |_, _| {
            let mut old_file = old_file.borrow_mut();
            if file.get_filename() != *old_file {
                *old_file = file.get_filename();
                self2.update();
            }
            if let Ok(crates) = rx.try_recv() {
                for old_result in packs.get_children() {
                    packs.remove(&old_result);
                }
                let mut results = results2.borrow_mut();
                *results = crates;
                store.clear();
                for c in &*results {
                    store.insert_with_values(None, &[0, 1, 2], &[&c.name, &c.description, &c.max_version]);
                }
                packs.show_all();
            }
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