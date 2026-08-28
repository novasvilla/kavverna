//! The catalogue as the settings page reads it.
//!
//! Everything here comes from the catalogue and the settings file rather than from a running
//! feature, so there is no thread behind it and nothing to publish.

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
        #[qproperty(QStringList, groups)]
        #[qproperty(QList_bool, installed)]
        #[qproperty(QList_bool, built)]
        #[qproperty(i32, installed_count)]
        #[qproperty(i32, built_count)]
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

#[derive(Default)]
pub struct FeaturesViewRust {
    ids: QStringList,
    titles: QStringList,
    summaries: QStringList,
    groups: QStringList,
    installed: QList<bool>,
    built: QList<bool>,
    installed_count: i32,
    built_count: i32,
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
        self.refresh();
    }

    fn refresh(mut self: Pin<&mut Self>) {
        let (mut ids, mut titles, mut summaries, mut groups) = (
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
            groups.append(QString::from(described.group.title()));
            installed.append(settings::is_installed(feature));
            built.append(feature.is_built());
        }

        let live = installed.iter().filter(|on| **on).count();
        let total = built.iter().filter(|is| **is).count();

        self.as_mut().set_ids(ids);
        self.as_mut().set_titles(titles);
        self.as_mut().set_summaries(summaries);
        self.as_mut().set_groups(groups);
        self.as_mut().set_installed(installed);
        self.as_mut().set_built(built);
        self.as_mut().set_installed_count(live as i32);
        self.as_mut().set_built_count(total as i32);
    }

    fn choose_installed(mut self: Pin<&mut Self>, id: &QString, installed: bool) {
        let wanted = id.to_string();
        let Some(feature) = Feature::iter().find(|feature| feature.id() == wanted) else {
            return;
        };
        if !feature.is_built() {
            return;
        }
        settings::set_installed(feature, installed);
        self.as_mut().refresh();
    }

    fn shows(&self, id: &QString) -> bool {
        let wanted = id.to_string();
        Feature::iter().find(|feature| feature.id() == wanted).is_some_and(settings::is_installed)
    }
}
