use crate::basis::{metadata::BasisSetDetail, BasisFile};
use periodic_table::periodic_table;
use tabled::Tabled;

#[derive(Tabled)]
pub(crate) struct BasisTableItem {
    #[tabled(rename = "Name")]
    pub name: String,
    #[tabled(rename = "Friendly names", display("format_slice"))]
    pub friendly_names: Vec<String>,
    #[tabled(rename = "Elements", display("format_slice"))]
    pub elements: Vec<String>,
}

fn format_slice(vec: &[String]) -> String {
    vec.chunks(3)
        .map(|chunk| chunk.join(", "))
        .collect::<Vec<String>>()
        .join("\n")
}

impl From<BasisSetDetail> for BasisTableItem {
    fn from(value: BasisSetDetail) -> Self {
        let mut friendly = vec![value.display_name.clone()];
        let elements = {
            value.get_latest_version().elements.iter().map(|el| {
                if let Ok(el_num) = el.parse::<usize>() {
                    periodic_table()[el_num - 1].symbol
                } else {
                    el.as_str()
                }
                .to_owned()
            })
        }
        .collect();
        friendly.extend(
            value
                .other_names
                .into_iter()
                .filter(|name| *name != value.display_name),
        );
        BasisTableItem {
            name: value.display_name,
            friendly_names: friendly,
            elements,
        }
    }
}

impl From<BasisFile> for BasisTableItem {
    fn from(value: BasisFile) -> Self {
        BasisTableItem {
            name: value.name,
            friendly_names: value.names,
            elements: value
                .elements
                .keys()
                .map(|index| periodic_table()[*index as usize - 1].symbol.to_owned())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::basis::{
        metadata::{BasisSetDetail, Version},
        BasisId,
    };

    use super::BasisTableItem;

    #[test]
    fn online_items_display_the_canonical_name() {
        let detail = BasisSetDetail {
            basename: BasisId::new("6-31g_st__st_").unwrap().into_owned(),
            description: String::new(),
            display_name: "6-31G**".into(),
            family: String::new(),
            function_types: vec![],
            latest_version: "1".into(),
            notes_exist: vec![],
            other_names: vec![],
            relpath: String::new(),
            role: String::new(),
            tags: vec![],
            versions: HashMap::from([(
                "1".into(),
                Version {
                    elements: vec![],
                    file_relpath: String::new(),
                    revdate: String::new(),
                    revdesc: String::new(),
                },
            )]),
        };

        assert_eq!(BasisTableItem::from(detail).name, "6-31G**");
    }
}
