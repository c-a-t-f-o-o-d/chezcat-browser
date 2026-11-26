//! Read-only range queries over the existing native boundary topology.
//! https://dom.spec.whatwg.org/#dom-range-compareboundarypoints
use super::*;
fn mode(args:&[Value])->Result<usize,String>{Ok(range_offset(args.first().unwrap_or(&Value::Undefined))? & 0xffff)}
fn numeric_order(order:Ordering)->Value {Value::Number(match order {Ordering::Less=>-1.0,Ordering::Equal=>0.0,Ordering::Greater=>1.0})}
impl Bridge<'_> {
    pub(super) fn validate_range_comparison(&mut self,receiver:u32,method:&str,args:&[Value])->Result<(),Error>{
        use Error::{Exception,Fatal};
        let record=*self.range_record(receiver).ok_or_else(||Exception("TypeError","A genuine Range receiver is required.".into()))?;
        if method==COMPARE_BOUNDARIES {
            let Some(Value::Host(source))=args.get(1) else {return Err(Exception("TypeError","A genuine source Range is required.".into()));};
            let source=*self.range_record(*source).ok_or_else(||Exception("TypeError","A genuine source Range is required.".into()))?;
            if mode(args).map_err(Fatal)?>3 {return Err(Exception("NotSupportedError","Unknown Range comparison mode.".into()));}
            if self.range_root(record.start.node).map_err(Fatal)?!=self.range_root(source.start.node).map_err(Fatal)? {
                return Err(Exception("WrongDocumentError","Range roots differ.".into()));
            }
            return Ok(());
        }
        let Some(Value::Host(node))=args.first() else {return Err(Exception("TypeError","A genuine Node is required.".into()));};
        let index=self.range_metadata_index(*node)?;
        if self.range_root(*node).map_err(Fatal)?!=self.range_root(record.start.node).map_err(Fatal)? {
            return if method==COMPARE_POINT {Err(Exception("WrongDocumentError","Point and Range roots differ.".into()))} else {Ok(())};
        }
        if method!=INTERSECTS_NODE {
            if index.is_some_and(|index|matches!(self.doc.nodes[index].kind,NodeKind::DocumentType(_))) {return Err(Exception("InvalidNodeTypeError","A DocumentType is not a boundary container.".into()));}
            let offset=range_offset(args.get(1).unwrap_or(&Value::Undefined)).map_err(Fatal)?;
            if offset>self.range_length(*node).map_err(Fatal)? {return Err(Exception("IndexSizeError","Point offset exceeds node length.".into()));}
        }
        Ok(())
    }
    pub(super) fn range_comparison(&mut self,record:Record,method:&str,args:&[Value])->Result<Value,String>{
        if method==COMPARE_BOUNDARIES {
            let Some(Value::Host(source))=args.get(1) else {return Err("Missing source Range.".into());};
            let source=*self.range_record(*source).ok_or("Source Range disappeared.")?;
            let (left,right)=match mode(args)? {
                0=>(record.start,source.start),1=>(record.end,source.start),
                2=>(record.end,source.end),3=>(record.start,source.end),
                _=>return Err("Range comparison mode was not validated.".into()),
            };
            return self.range_compare(left,right)?.map(numeric_order).ok_or_else(||"Range roots differ.".into());
        }
        let Some(Value::Host(node))=args.first() else {return Err("Missing point Node.".into());};
        if self.range_root(*node)?!=self.range_root(record.start.node)? {return Ok(Value::Bool(false));}
        if method==INTERSECTS_NODE {
            if self.range_parent(*node)?.is_none() {return Ok(Value::Bool(true));}
            let(parent,offset)=self.range_index(*node)?;
            return Ok(Value::Bool(self.range_compare(Point{node:parent,offset},record.end)?==Some(Ordering::Less)
                && self.range_compare(Point{node:parent,offset:offset+1},record.start)?==Some(Ordering::Greater)));
        }
        let point=Point{node:*node,offset:range_offset(args.get(1).unwrap_or(&Value::Undefined))?};
        let order=if self.range_compare(point,record.start)?==Some(Ordering::Less) {Ordering::Less}
            else if self.range_compare(point,record.end)?==Some(Ordering::Greater) {Ordering::Greater}else {Ordering::Equal};
        Ok(if method==POINT_IN_RANGE {Value::Bool(order==Ordering::Equal)}else{numeric_order(order)})
    }
}
