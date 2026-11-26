//! Original strict Unicode property parsing and static range lookup.
use super::*;

#[path = "regexp_unicode_property_data.rs"]
mod data;

#[path = "regexp_unicode_emoji_data.rs"]
mod emoji;

pub(super) fn contains(ranges: &[(u32,u32)], character: u32) -> bool {
    let after = ranges.partition_point(|&(low, _)| low <= character);
    after != 0 && character <= ranges[after - 1].1
}

fn lookup(group: &[(&str,usize)], name: &str, budget: &mut usize) -> Result<Option<&'static [(u32,u32)]>, Error> {
    let factor = (usize::BITS - group.len().leading_zeros()).max(1) as usize;
    spend(budget, name.len().checked_mul(factor).ok_or(Error::Limit("Unicode regexp property work overflow."))?)?;
    Ok(group.binary_search_by_key(&name, |entry| entry.0).ok().map(|index| data::TABLES[group[index].1]))
}

impl Parser<'_> {
    pub(super) fn property_escape(&mut self, inverted: bool) -> Result<ClassPart, Error> {
        if !self.eat(123) { return Err(Error::Syntax("A Unicode regexp property requires braces.")); }
        let start = self.at;
        while self.peek().is_some_and(|unit| matches!(unit, 48..=57 | 65..=90 | 97..=122 | 95 | 61)) {
            self.at += 1;
        }
        let end = self.at;
        if start == end || !self.eat(125) { return Err(Error::Syntax("Invalid Unicode regexp property expression.")); }
        let length = end - start;
        spend(self.budget, length)?;
        let mut name = String::new();
        name.try_reserve_exact(length).map_err(|_| Error::Limit("Unicode regexp property storage is unavailable."))?;
        spend(self.budget, name.capacity())?;
        for &unit in &self.units[start..end] { name.push(unit as u8 as char); }
        spend(self.budget, name.len().checked_mul(3).ok_or(Error::Limit("Unicode regexp property work overflow."))?)?;
        if let Ok(index) = emoji::PROPERTIES.binary_search_by_key(&name.as_str(), |entry| entry.0) {
            if !self.unicode_sets || inverted {
                return Err(Error::Syntax("A Unicode string property requires v and cannot be complemented."));
            }
            return Ok(ClassPart::StringProperty(emoji::PROPERTIES[index].1));
        }
        let ranges = if let Some((property, value)) = name.split_once('=') {
            let group = match property {
                "gc" | "General_Category" => data::CATEGORY,
                "sc" | "Script" => data::SCRIPT,
                "scx" | "Script_Extensions" => data::SCRIPT_EXTENSIONS,
                _ => return Err(Error::Syntax("Invalid Unicode regexp property name.")),
            };
            lookup(group, value, self.budget)?
        } else {
            match lookup(data::CATEGORY, &name, self.budget)? {
                Some(ranges) => Some(ranges),
                None => lookup(data::BINARY, &name, self.budget)?,
            }
        };
        ranges.map(|ranges| ClassPart::Property(ranges, inverted)).ok_or(Error::Syntax("Invalid Unicode regexp property name or value."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_emoji_properties_static_sequences_and_union_are_exact() {
        assert_eq!(emoji::MEMBERS.len(),3953);
        for pair in emoji::MEMBERS.windows(2) { assert!(pair[0] < pair[1]); }
        for value in emoji::MEMBERS {
            assert!(!value.is_empty());
            assert!(value.iter().all(|&code| char::from_u32(code).is_some()));
        }
        let mut union=std::collections::BTreeSet::new();
        let mut rgi=None;
        for &(name,values) in emoji::PROPERTIES {
            for pair in values.windows(2) { assert!(pair[0] < pair[1]); }
            for value in values { assert!(emoji::MEMBERS.binary_search(value).is_ok()); }
            if name=="RGI_Emoji" {rgi=Some(values);} else {union.extend(values.iter().copied());}
        }
        assert_eq!(emoji::PROPERTIES.len(),7);
        assert_eq!(union.into_iter().collect::<Vec<_>>(),rgi.unwrap());
        for pair in emoji::PROPERTIES.windows(2) {assert!(pair[0].0<pair[1].0);}
    }
    #[test]
    fn unicode_property_tables_are_ordered_and_match_existing_identifier_data() {
        for table in data::TABLES {
            for &(low,high) in *table { assert!(low<=high && high<=0x10ffff); }
            for pair in table.windows(2) { assert!(pair[0].1 < pair[1].0); }
        }
        for group in [data::CATEGORY,data::SCRIPT,data::SCRIPT_EXTENSIONS,data::BINARY] {
            for pair in group.windows(2) { assert!(pair[0].0 < pair[1].0); }
            for &(_,index) in group { assert!(index<data::TABLES.len()); }
        }
        let start=lookup(data::BINARY,"ID_Start",&mut 100_000).unwrap().unwrap();
        let part=lookup(data::BINARY,"ID_Continue",&mut 100_000).unwrap().unwrap();
        for code in 0..=0x10ffff {
            let Some(character)=char::from_u32(code) else {continue};
            assert_eq!(contains(start,code),super::super::super::identifier_names::start(character) && !matches!(character,'$'|'_'),"ID_Start U+{code:x}");
            assert_eq!(contains(part,code),super::super::super::identifier_names::part(character) && character!='$',"ID_Continue U+{code:x}");
        }
    }
}
