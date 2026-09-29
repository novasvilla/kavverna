//! The catalogue as the settings page reads it.
//!
//! Everything here comes from the catalogue and the settings file rather than from a running
//! feature, so there is no thread behind it and nothing to publish.

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QList, QString, QStringList};
use feature_catalog::Feature;
use strum::IntoEnumIterator;

use crate::settings;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_bool = cxx_qt_lib::QList<bool>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QStringList, ids)]
        #[qproperty(QStringList, titles)]
        #[qproperty(QStringList, summaries)]
        #[qproperty(QStringList, energies)]
        #[qproperty(QStringList, groups)]
        #[qproperty(QList_bool, installed)]
        #[qproperty(QList_bool, built)]
        #[qproperty(i32, installed_count)]
        #[qproperty(i32, built_count)]
        #[qproperty(bool, restart_required)]
        #[qproperty(bool, settings_writable)]
        #[qproperty(QString, save_notice)]
        type FeaturesView = super::FeaturesViewRust;
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut FeaturesView>);
        #[qinvokable]
        fn choose_installed(self: Pin<&mut FeaturesView>, id: &QString, installed: bool);
    }
}

use core::pin::Pin;
use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

#[derive(Default)]
pub struct FeaturesViewRust {
    ids: QStringList,
    titles: QStringList,
    summaries: QStringList,
    energies: QStringList,
    groups: QStringList,
    installed: QList<bool>,
    built: QList<bool>,
    installed_count: i32,
    built_count: i32,
    restart_required: bool,
    settings_writable: bool,
    save_notice: QString,
    active: Vec<bool>,
}

/// Grouped the way the catalogue groups them, and in the panel's own order within a group, so
/// the list reads as the same product as the panel rather than an alphabetical inventory.
fn in_display_order() -> Vec<Feature> {
    let mut features: Vec<_> = Feature::iter().collect();
    features.sort_by_key(|feature| (feature.describe().group, *feature));
    features
}

impl qobject::FeaturesView {
    fn attach(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut().rust_mut().get_mut().active =
            in_display_order().into_iter().map(settings::is_installed).collect();
        this.as_mut().refresh();
    }

    fn refresh(mut self: Pin<&mut Self>) {
        let (mut ids, mut titles, mut summaries, mut energies, mut groups) = (
            QStringList::default(),
            QStringList::default(),
            QStringList::default(),
            QStringList::default(),
            QStringList::default(),
        );
        let (mut installed, mut built) = (QList::<bool>::default(), QList::<bool>::default());

        for feature in in_display_order() {
            let described = feature.describe();
            ids.append(QString::from(feature.id()));
            titles.append(QString::from(described.title));
            summaries.append(QString::from(described.summary));
            energies.append(QString::from(described.energy.label()));
            groups.append(QString::from(described.group.title()));
            installed.append(settings::is_installed(feature));
            built.append(feature.is_built());
        }

        let live = installed.iter().filter(|on| **on).count();
        let total = built.iter().filter(|is| **is).count();

        self.as_mut().set_ids(ids);
        self.as_mut().set_titles(titles);
        self.as_mut().set_summaries(summaries);
        self.as_mut().set_energies(energies);
        self.as_mut().set_groups(groups);
        self.as_mut().set_installed(installed);
        self.as_mut().set_built(built);
        self.as_mut().set_installed_count(live as i32);
        self.as_mut().set_built_count(total as i32);
        self.as_mut().set_settings_writable(settings::can_save());
        let active = &self.rust().active;
        let restart_required =
            in_display_order().into_iter().enumerate().any(|(index, feature)| {
                active.get(index).is_some_and(|was| *was != settings::is_installed(feature))
            });
        self.as_mut().set_restart_required(restart_required);
    }

    fn choose_installed(mut self: Pin<&mut Self>, id: &QString, installed: bool) {
        let wanted = id.to_string();
        let Some(feature) = Feature::iter().find(|feature| feature.id() == wanted) else {
            return;
        };
        if !feature.is_built() {
            return;
        }
        if settings::is_installed(feature) == installed {
            return;
        }
        let saved = settings::set_installed(feature, installed);
        self.as_mut().set_save_notice(QString::from(if saved {
            ""
        } else {
            "The utility selection could not be saved."
        }));
        self.as_mut().refresh();
        if saved {
            let err = relaunch();
            tracing::error!(%err, "Kavverna could not restart after a utility changed");
            self.as_mut().set_save_notice(QString::from(
                "The selection was saved, but Kavverna could not restart.",
            ));
        }
    }
}

fn relaunch_executable(
    current: io::Result<PathBuf>,
    invoked_as: Option<OsString>,
) -> io::Result<OsString> {
    match current {
        Ok(path) if path.is_file() => Ok(path.into_os_string()),
        _ => invoked_as.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Kavverna executable is no longer available")
        }),
    }
}

fn relaunch() -> io::Error {
    let program = match relaunch_executable(std::env::current_exe(), std::env::args_os().next()) {
        Ok(program) => program,
        Err(err) => return err,
    };
    if let Err(err) = prepare_descriptors_for_restart() {
        return err;
    }
    Command::new(program).arg("--settings").exec()
}

fn prepare_descriptors_for_restart() -> io::Result<()> {
    // Qt can leave a GPU render descriptor without CLOEXEC. Isolating this thread's descriptor
    // table also keeps another worker from opening a new inheritable descriptor before exec.
    let flags = (libc::CLOSE_RANGE_CLOEXEC | libc::CLOSE_RANGE_UNSHARE) as libc::c_int;
    let status = unsafe { libc::close_range(3, u32::MAX, flags) };
    if status == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_binary_uses_the_name_that_launched_kavverna() {
        let old_image = PathBuf::from("/no-longer-present/kavverna-shell (deleted)");
        let launch = OsString::from("kavverna-shell");

        assert_eq!(relaunch_executable(Ok(old_image), Some(launch.clone())).unwrap(), launch);
    }

    #[test]
    fn a_restart_does_not_inherit_open_descriptors() {
        use std::os::fd::AsRawFd;

        let file = std::fs::File::open("/dev/null").unwrap();
        let fd = file.as_raw_fd();
        let original = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(original >= 0);
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFD, original & !libc::FD_CLOEXEC) }, 0);
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC, 0);

        prepare_descriptors_for_restart().unwrap();

        assert_ne!(unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC, 0);
    }
}
