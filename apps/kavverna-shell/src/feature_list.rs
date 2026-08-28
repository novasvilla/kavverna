use cxx_qt_lib::{QString, QStringList};
use feature_catalog::{Feature, Group};
use strum::IntoEnumIterator;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QStringList, titles)]
        #[qproperty(QStringList, summaries)]
        #[qproperty(QString, heading)]
        type FeatureList = super::FeatureListRust;
    }
}

pub struct FeatureListRust {
    titles: QStringList,
    summaries: QStringList,
    heading: QString,
}

impl Default for FeatureListRust {
    fn default() -> Self {
        let mut titles = QStringList::default();
        let mut summaries = QStringList::default();

        for feature in Feature::iter() {
            let descriptor = feature.describe();
            titles.append(QString::from(descriptor.title));
            summaries.append(QString::from(descriptor.summary));
        }

        let groups = [Group::Sound, Group::Monitoring, Group::Clipboard, Group::Energy];
        let heading = QString::from(&format!(
            "{} features across {} groups",
            Feature::iter().count(),
            groups.len()
        ));

        Self { titles, summaries, heading }
    }
}
