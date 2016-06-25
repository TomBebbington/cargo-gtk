use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Window, FileChooser, Button, Label, Dialog, SpinButton, Builder};

use cargo::util::config::Config;
use cargo::ops::{self, CompileOptions, CompileMode};
use cargo::core::package::Package;

use util::{Options, PreContext};
use pages::Page;


fn update_labels(labels: &[(&Label, &str)]) {
    for &(label, text) in labels {
        label.set_text(text);
    }
}

/// An interface for configuring compilation options.
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
        let builder = Builder::new_from_string(include_str!("../compile.glade"));
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


/// An interface for building and viewing local dev packages.
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
        ::bind_file_button(&self.file, button, move |file| {
            let options = options.borrow();
            let mut options: CompileOptions = (&*options).into();
            options.mode = mode;

            op!(
                &window,
                ops::compile(&file, &options),
                ("Successfully ran subcommand {}", name),
                "Failed to create crate due to {:?}"
            );
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