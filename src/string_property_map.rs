//! Exact ordinary string keys with allocation-free scalar lookups.
use super::{Property, PropertyKey, Value};
use std::collections::{btree_map, BTreeMap};
use std::ops::Index;

// Unit keys contain at least one unpaired code unit. Conversion canonicalizes
// well-formed strings to Scalar before they reach this storage boundary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum StringKey {
    Scalar(String),
    Units(Vec<u16>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum StringKeyRef<'a> {
    Scalar(&'a str),
    Units(&'a [u16]),
}

impl StringKey {
    pub(super) fn as_ref(&self) -> StringKeyRef<'_> {
        match self {
            Self::Scalar(name) => StringKeyRef::Scalar(name),
            Self::Units(units) => StringKeyRef::Units(units),
        }
    }

    pub(super) fn byte_len(&self) -> usize { self.as_ref().byte_len() }

    pub(super) fn capacity_bytes(&self) -> usize {
        match self {
            Self::Scalar(name) => name.capacity(),
            Self::Units(units) => units.capacity().saturating_mul(2),
        }
    }

    pub(super) fn as_scalar(&self) -> Option<&str> { self.as_ref().as_scalar() }

    pub(super) fn index(&self) -> Option<usize> { self.as_ref().index() }

    pub(super) fn into_value(self) -> Value {
        match self {
            Self::Scalar(name) => Value::String(name),
            Self::Units(units) => Value::StringUnits(units),
        }
    }

    pub(super) fn into_property_key(self) -> PropertyKey {
        match self {
            Self::Scalar(name) => PropertyKey::String(name),
            Self::Units(units) => PropertyKey::StringUnits(units),
        }
    }
}

impl<'a> StringKeyRef<'a> {
    pub(super) fn byte_len(self) -> usize {
        match self {
            Self::Scalar(name) => name.len(),
            Self::Units(units) => units.len().saturating_mul(2),
        }
    }

    pub(super) fn as_scalar(self) -> Option<&'a str> {
        match self {
            Self::Scalar(name) => Some(name),
            Self::Units(_) => None,
        }
    }

    pub(super) fn index(self) -> Option<usize> {
        self.as_scalar().and_then(super::property_index)
    }

    pub(super) fn to_owned(self) -> StringKey {
        match self {
            Self::Scalar(name) => StringKey::Scalar(name.to_owned()),
            Self::Units(units) => StringKey::Units(units.to_vec()),
        }
    }
}

impl From<String> for StringKey {
    fn from(name: String) -> Self { Self::Scalar(name) }
}

impl From<&str> for StringKey {
    fn from(name: &str) -> Self { Self::Scalar(name.to_owned()) }
}

#[derive(Default)]
pub(super) struct StringProperties {
    scalar: BTreeMap<String, Property>,
    units: BTreeMap<Vec<u16>, Property>,
}

impl StringProperties {
    pub(super) fn new() -> Self { Self::default() }

    pub(super) fn len(&self) -> usize { self.scalar.len() + self.units.len() }

    pub(super) fn is_empty(&self) -> bool { self.scalar.is_empty() && self.units.is_empty() }

    pub(super) fn get(&self, name: &str) -> Option<&Property> { self.scalar.get(name) }

    pub(super) fn get_mut(&mut self, name: &str) -> Option<&mut Property> { self.scalar.get_mut(name) }

    pub(super) fn contains_key(&self, name: &str) -> bool { self.scalar.contains_key(name) }

    pub(super) fn insert(&mut self, name: String, property: Property) -> Option<Property> {
        self.scalar.insert(name, property)
    }

    pub(super) fn remove(&mut self, name: &str) -> Option<Property> { self.scalar.remove(name) }

    pub(super) fn get_units(&self, units: &[u16]) -> Option<&Property> { self.units.get(units) }

    pub(super) fn get_mut_units(&mut self, units: &[u16]) -> Option<&mut Property> {
        self.units.get_mut(units)
    }

    pub(super) fn insert_units(&mut self, units: Vec<u16>, property: Property) -> Option<Property> {
        self.units.insert(units, property)
    }

    pub(super) fn remove_units(&mut self, units: &[u16]) -> Option<Property> { self.units.remove(units) }

    pub(super) fn get_key(&self, key: &PropertyKey) -> Option<&Property> {
        match key {
            PropertyKey::String(name) => self.get(name),
            PropertyKey::StringUnits(units) => self.get_units(units),
            PropertyKey::Symbol(_) => None,
        }
    }

    pub(super) fn get_key_mut(&mut self, key: &PropertyKey) -> Option<&mut Property> {
        match key {
            PropertyKey::String(name) => self.get_mut(name),
            PropertyKey::StringUnits(units) => self.get_mut_units(units),
            PropertyKey::Symbol(_) => None,
        }
    }

    // All value visitors include raw-keyed data and accessors, including the
    // reachability walker and the seal/freeze checks.
    pub(super) fn values(&self) -> impl Iterator<Item = &Property> {
        self.scalar.values().chain(self.units.values())
    }

    pub(super) fn values_mut(&mut self) -> impl Iterator<Item = &mut Property> {
        self.scalar.values_mut().chain(self.units.values_mut())
    }

    // Storage iteration is not JavaScript property order. Callers must sort
    // integer indices and Property::order before exposing an own-key snapshot.
    pub(super) fn iter(&self) -> Iter<'_> {
        Iter { scalar: self.scalar.iter(), units: self.units.iter() }
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = StringKeyRef<'_>> {
        self.iter().map(|(key, _)| key)
    }

    pub(super) fn scalar_iter(&self) -> btree_map::Iter<'_, String, Property> { self.scalar.iter() }

    pub(super) fn scalar_keys(&self) -> btree_map::Keys<'_, String, Property> { self.scalar.keys() }
}

impl Index<&str> for StringProperties {
    type Output = Property;
    fn index(&self, name: &str) -> &Self::Output { &self.scalar[name] }
}

impl Index<&String> for StringProperties {
    type Output = Property;
    fn index(&self, name: &String) -> &Self::Output { &self.scalar[name] }
}

pub(super) struct Iter<'a> {
    scalar: btree_map::Iter<'a, String, Property>,
    units: btree_map::Iter<'a, Vec<u16>, Property>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (StringKeyRef<'a>, &'a Property);

    fn next(&mut self) -> Option<Self::Item> {
        self.scalar.next().map(|(key, property)| (StringKeyRef::Scalar(key), property))
            .or_else(|| self.units.next().map(|(key, property)| (StringKeyRef::Units(key), property)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.scalar.len() + self.units.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for Iter<'_> {}
impl std::iter::FusedIterator for Iter<'_> {}

impl<'a> IntoIterator for &'a StringProperties {
    type Item = (StringKeyRef<'a>, &'a Property);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}

pub(super) struct IntoIter {
    scalar: btree_map::IntoIter<String, Property>,
    units: btree_map::IntoIter<Vec<u16>, Property>,
}

impl Iterator for IntoIter {
    type Item = (StringKey, Property);

    fn next(&mut self) -> Option<Self::Item> {
        self.scalar.next().map(|(key, property)| (StringKey::Scalar(key), property))
            .or_else(|| self.units.next().map(|(key, property)| (StringKey::Units(key), property)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.scalar.len() + self.units.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for IntoIter {}
impl std::iter::FusedIterator for IntoIter {}

impl IntoIterator for StringProperties {
    type Item = (StringKey, Property);
    type IntoIter = IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter { scalar: self.scalar.into_iter(), units: self.units.into_iter() }
    }
}
