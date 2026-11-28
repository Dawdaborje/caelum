// SPDX-License-Identifier: GPL-3.0-or-later
//! An item's context menu over `com.canonical.dbusmenu`.
use std::collections::HashMap;

use serde::Deserialize;
use zbus::zvariant::{OwnedValue, Type, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub toggle: Toggle,
    pub children: Vec<MenuItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    None,
    Check(bool),
    Radio(bool),
}

#[zbus::proxy(interface = "com.canonical.dbusmenu")]
pub trait DBusMenu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<(u32, Layout)>;

    fn event(&self, id: i32, event_id: &str, data: &Value<'_>, timestamp: u32) -> zbus::Result<()>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;
}

#[derive(Debug, Deserialize, Type)]
pub struct Layout(i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

impl Layout {
    /// The menu tree, without invisible entries.
    pub fn into_items(self) -> Vec<MenuItem> {
        self.2
            .into_iter()
            .filter_map(|child| {
                let s = child.downcast_ref::<zbus::zvariant::Structure>().ok()?;
                Layout::try_from(&s).ok()?.into_item()
            })
            .collect()
    }

    fn into_item(self) -> Option<MenuItem> {
        let props = &self.1;
        let get = |k: &str| props.get(k).map(|v| &**v);
        let flag = |k: &str| match get(k) {
            Some(Value::Bool(b)) => *b,
            _ => true,
        };
        let text = |k: &str| match get(k) {
            Some(Value::Str(s)) => s.as_str().to_owned(),
            _ => String::new(),
        };
        if !flag("visible") {
            return None;
        }
        let on = matches!(get("toggle-state"), Some(Value::I32(1)));
        let toggle = match text("toggle-type").as_str() {
            "checkmark" => Toggle::Check(on),
            "radio" => Toggle::Radio(on),
            _ => Toggle::None,
        };
        Some(MenuItem {
            id: self.0,
            label: strip_mnemonic(&text("label")),
            enabled: flag("enabled"),
            separator: text("type") == "separator",
            toggle,
            children: self.into_items(),
        })
    }
}

impl TryFrom<&zbus::zvariant::Structure<'_>> for Layout {
    type Error = zbus::zvariant::Error;
    fn try_from(s: &zbus::zvariant::Structure<'_>) -> Result<Self, Self::Error> {
        let f = s.fields();
        let bad = || zbus::zvariant::Error::IncorrectType;
        let id = i32::try_from(f.first().ok_or_else(bad)?)?;
        let props =
            HashMap::<String, OwnedValue>::try_from(f.get(1).ok_or_else(bad)?.try_clone()?)?;
        let children = Vec::<OwnedValue>::try_from(f.get(2).ok_or_else(bad)?.try_clone()?)?;
        Ok(Layout(id, props, children))
    }
}

/// dbusmenu labels mark the access key with `_`; `__` is a literal underscore.
fn strip_mnemonic(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            if chars.peek() == Some(&'_') {
                out.push('_');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics_are_stripped() {
        assert_eq!(strip_mnemonic("_Quit"), "Quit");
        assert_eq!(strip_mnemonic("snake__case"), "snake_case");
    }
}
