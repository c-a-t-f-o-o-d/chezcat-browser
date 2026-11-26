//! Original Unicode RegExp canonicalization over pinned simple/common mappings.
use super::*;

#[path = "regexp_unicode_folding_data.rs"]
mod data;

pub(super) fn mappings() -> &'static [(u32,u32)] { data::PAIRS }

pub(super) fn canonicalize(character: u32, budget: &mut usize) -> Result<u32, Error> {
    spend(budget,(usize::BITS-data::PAIRS.len().leading_zeros()).max(1) as usize)?;
    Ok(data::PAIRS.binary_search_by_key(&character,|entry|entry.0)
        .ok().map_or(character,|index|data::PAIRS[index].1))
}

pub(super) fn equal(left: u32, right: u32, flags: Flags, budget: &mut usize) -> Result<bool, Error> {
    if left==right {return Ok(true);}
    if !flags.insensitive {return Ok(false);}
    if flags.unicode {Ok(canonicalize(left,budget)?==canonicalize(right,budget)?)}
    else {Ok(ascii_fold(left)==ascii_fold(right))}
}

pub(super) fn word_character(character: Option<u32>, flags: Flags, budget: &mut usize) -> Result<bool, Error> {
    let Some(character)=character else {return Ok(false)};
    let character=if flags.unicode&&flags.insensitive {canonicalize(character,budget)?}else{character};
    Ok(word(character))
}

pub(super) fn same_units(left: &[u16], right: &[u16], flags: Flags, budget: &mut usize) -> Result<bool, Error> {
    let (mut a,mut b)=(0,0);
    while a<left.len()&&b<right.len() {
        spend(budget,1)?;
        let (first,first_width)=character_at(left,a,flags.unicode).unwrap();
        let (second,second_width)=character_at(right,b,flags.unicode).unwrap();
        if !equal(first,second,flags,budget)? {return Ok(false);}
        a+=first_width;b+=second_width;
    }
    Ok(a==left.len()&&b==right.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_case_folding_data_is_ordered_idempotent_and_width_preserving() {
        assert_eq!(data::PAIRS.len(),1512);
        for pair in data::PAIRS.windows(2) {assert!(pair[0].0<pair[1].0);}
        for &(source,target) in data::PAIRS {
            let source_char=char::from_u32(source).unwrap();let target_char=char::from_u32(target).unwrap();
            assert_eq!(source_char.len_utf16(),target_char.len_utf16());
            assert_eq!(canonicalize(source,&mut 100).unwrap(),target);
            assert_eq!(canonicalize(target,&mut 100).unwrap(),target);
            assert_ne!(source,target);
        }
    }
}
