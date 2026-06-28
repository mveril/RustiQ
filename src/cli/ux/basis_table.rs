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
            name: value.basename,
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
