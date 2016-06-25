use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Window, FileChooser, Button, Entry, ComboBoxText};

use cargo::util::config::Config;
use cargo::ops::{self, VersionControl, NewOptions};

use util::PreContext;
use pages::Page;

/// An interface for creating packages.
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
        ::bind_file_button(&self.file, &self.new, move |file| {
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
                ::info(Some(&window), "Created crate successfully");
            } else {
                ::error(Some(&window), "Failed to create crate");
            }
        });
    }
}
