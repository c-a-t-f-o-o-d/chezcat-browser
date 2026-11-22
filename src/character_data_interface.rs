//! Canonical CharacterData editing operations and WebIDL conversion order.
#[derive(Clone,Copy,Debug)]
pub(crate) enum Edit{Append,Insert,Delete,Replace}
pub(crate) const METHODS:[(&str,&str,usize);4]=[("appendData","\0CharacterData.appendData",1),("insertData","\0CharacterData.insertData",2),("deleteData","\0CharacterData.deleteData",2),("replaceData","\0CharacterData.replaceData",3)];
pub(crate) fn operation(method:&str)->Option<Edit>{Some(match method{"\0CharacterData.appendData"=>Edit::Append,"\0CharacterData.insertData"=>Edit::Insert,"\0CharacterData.deleteData"=>Edit::Delete,"\0CharacterData.replaceData"=>Edit::Replace,_=>return None})}
impl Edit{
 pub(crate) fn arity(self)->usize{match self{Self::Append=>1,Self::Insert|Self::Delete=>2,Self::Replace=>3}}
 pub(crate) fn data_index(self)->Option<usize>{match self{Self::Append=>Some(0),Self::Insert=>Some(1),Self::Replace=>Some(2),Self::Delete=>None}}
 pub(crate) fn conversions(self)->&'static[(usize,bool)]{match self{Self::Append=>&[(0,true)],Self::Insert=>&[(0,false),(1,true)],Self::Delete=>&[(0,false),(1,false)],Self::Replace=>&[(0,false),(1,false),(2,true)]}}
}
