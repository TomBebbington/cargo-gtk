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
use std::marker::PhantomData;


use thread_scoped::scoped;

use cargo::ops::{self, ExecEngine, CompileOptions, CompileFilter, CompileMode};
use cargo::core::package::Package;
use cargo::util::config::Config;
use cargo::core::source::SourceId;

use crates_io::{Crate, Registry};

use gtk::prelude::*;
use gtk::{DIALOG_DESTROY_WITH_PARENT, DIALOG_MODAL, ButtonsType, Label, Builder, MessageType, MessageDialog, CellRendererText, FileChooser, TreeView, Button, ListStore, TreeViewColumn, Window, SearchEntry};

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

fn error(parent: Option<&Window>, text: &str) {
    let dialog = MessageDialog::new(parent, DIALOG_DESTROY_WITH_PARENT | DIALOG_MODAL, MessageType::Error, ButtonsType::Close, text);
    dialog.connect_response(|dialog, _| {
        dialog.destroy();
    });
    dialog.set_icon_name(Some("dialog_error"));
    dialog.set_deletable(true);
    dialog.show_all();
    dialog.present();
    dialog.run();
}

macro_rules! error {
    ($window:expr, $($tt:tt)*) => (error($window, &format!($($tt)*)))
}

pub struct Options<'a, F> where F: Fn() -> CompileFilter<'a> + 'a {
    pub config: Config,
    pub jobs: Option<u32>,
    pub target: Option<String>,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub spec: Vec<String>,
    pub filter: F,
    pub exec_engine: Option<Arc<Box<ExecEngine>>>,
    pub release: bool,
    pub mode: CompileMode,
    pub target_rustdoc_args: Option<Vec<String>>,
    pub target_rustc_args: Option<Vec<String>>,
    data: PhantomData<& 'a ()>
}
impl<'a, F> Options<'a, F> where F: Fn() -> CompileFilter<'a> + 'a {
    pub fn new(filter: F) -> Options<'a, F> where F: Fn() -> CompileFilter<'a> + 'a {
        Options {
            jobs: Some(8),
            features: Vec::new(),
            spec: Vec::new(),
            config: Config::default().unwrap(),
            exec_engine: None,
            release: false,
            filter: filter,
            no_default_features: false,
            target: None,
            target_rustdoc_args: None,
            target_rustc_args: None,
            mode: CompileMode::Build,
            data: PhantomData
        }
    }
}
impl<'a, F> Into<CompileOptions<'a>> for &'a Options<'a, F> where F: Fn() -> CompileFilter<'a> + 'a {
    fn into(self) -> CompileOptions<'a> {
        CompileOptions {
            config: &self.config,
            jobs: self.jobs,
            target: self.target.as_ref().map(|v| v as &str),
            features: &self.features,
            no_default_features: self.no_default_features,
            spec: &self.spec,
            filter: (self.filter)(),
            exec_engine: self.exec_engine.clone(),
            release: self.release,
            mode: self.mode,
            target_rustdoc_args: self.target_rustdoc_args.as_ref().map(|v| v as &[String]),
            target_rustc_args: self.target_rustc_args.as_ref().map(|v| v as &[String])
        }
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
            local_name: builder.get_object("local_name").unwrap(),
            local_version: builder.get_object("local_version").unwrap(),
            local_author: builder.get_object("local_author").unwrap(),
            local_description: builder.get_object("local_description").unwrap(),
            online_search: builder.get_object("online_search").unwrap(),
            online_packages: packs
        }
    }
    fn bind_button<A>(&self, name: &'static str, button: &Button, action: A) where A: Fn(&Path) + 'static {
        let file = self.file.clone();
        let window = self.window.clone();
        button.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                action(&file);
            } else {
                error!(Some(&window), "A Cargo.toml package must be selected to run the '{}' command", name);
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
        let file = self.file.clone();


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
        let window = self.window.clone();
        let config2 = self.config.clone();
        self.install.connect_clicked(move |_| {
            let index = packs2.get_cursor().0.unwrap().get_indices()[0];
            let results = results.borrow();
            let id = SourceId::for_central(&*config2).unwrap();
            let options = Options::new(|| CompileFilter::Everything);
            let options: CompileOptions = (&options).into();
            let name = &results[index as usize].name;
            if let Err(err) = ops::install(None, Some(name), &id, None, &options) {
                error!(Some(&window), "Failed to install '{}': {:?}", name, err);
            }
        });
        let window2 = self.window.clone();
        self.run.connect_clicked(move |_| {
            if let Some(file) = file.get_filename() {
                let options = Options::new(|| CompileFilter::Everything);
                let options: CompileOptions = (&options).into();
                if let Err(err) = ops::run(&file, &options, &[]) {
                    error!(Some(&window2), "Failed to run '{:?}': {:?}", file, err);
                }
            }
        });
        let window = self.window.clone();
        let config3 = self.config.clone();
        self.bind_button("publish", &self.publish, move |path| {
            let package = Package::for_path(&path, &*config3).unwrap();
            if !package.publish() {
                error(Some(&window), "Failed to publish crate");
            }
        });
        /*
        self.bind_buttons(&[
            (&self.build, "cargo build"),
            (&self.run, "cargo run"),
            (&self.bench, "cargo bench"),
            (&self.publish, "cargo publish"),
            (&self.update, "cargo update"),
            (&self.doc, "cargo doc"),
            (&self.test, "cargo test")
        ]);*/
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