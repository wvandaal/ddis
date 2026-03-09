//! `braid query` — Query the store by entity and/or attribute.

use std::path::Path;

use braid_kernel::datom::{Attribute, EntityId, Op};

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    entity_filter: Option<&str>,
    attribute_filter: Option<&str>,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let entity_id = entity_filter.map(EntityId::from_ident);
    let attr = attribute_filter.map(Attribute::from_keyword);

    let mut out = String::new();
    let mut count = 0;

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Some(eid) = entity_id {
            if datom.entity != eid {
                continue;
            }
        }
        if let Some(ref a) = attr {
            if datom.attribute != *a {
                continue;
            }
        }

        out.push_str(&format!(
            "[{:?} {} {:?}]\n",
            datom.entity,
            datom.attribute.as_str(),
            datom.value,
        ));
        count += 1;
    }

    out.push_str(&format!("\n{count} datom(s)\n"));
    Ok(out)
}
