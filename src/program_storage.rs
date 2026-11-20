//! Immutable Program backing. Shared regexp code and mutable tagged-site realm
//! caches have independent lifetime accounting; this snapshot never refunds
//! them when a registry-only Program retires.
use super::*;
fn arena<T>(values:&Vec<T>)->usize{values.capacity().saturating_mul(std::mem::size_of::<T>())}
fn literal(value:&Value)->usize {
    match value {
        Value::String(text)=>text.capacity(),
        Value::StringUnits(units)=>arena(units),
        Value::BigInt(text)=>text.len().saturating_add(2*std::mem::size_of::<usize>()),
        Value::Undefined|Value::Null|Value::Bool(_)|Value::Number(_)|Value::Symbol(_)|Value::Function(_)|Value::Object(_)|Value::Host(_)=>0,
    }
}
impl Program {
    pub(super) fn retained_payload_bytes(&self,mut tick:impl FnMut()->Result<(),Fault>)->Result<usize,Fault> {
        let mut bytes=std::mem::size_of::<Self>()+2*std::mem::size_of::<usize>()+self.source.capacity();
        for backing in [arena(&self.exprs),arena(&self.stmts),arena(&self.templates),arena(&self.classes),arena(&self.patterns),arena(&self.body)]{bytes=bytes.saturating_add(backing);}
        for expression in &self.exprs {
            tick()?;
            let owned=match &expression.kind {
                ExprKind::Literal(value)=>literal(value),
                ExprKind::TemplateLiteral(first,parts)=>{
                    let mut bytes=first.capacity().saturating_add(arena(parts));
                    for (_,text) in parts{tick()?;bytes=bytes.saturating_add(text.capacity());}bytes
                }
                ExprKind::TemplateObject(site)=>site.immutable_bytes(&mut tick)?,
                ExprKind::Name(name)|ExprKind::SuperMember(name)=>name.capacity(),
                ExprKind::SuperCall(args)|ExprKind::Call(_,args)|ExprKind::Construct(_,args)=>arena(args),
                ExprKind::OptionalChain(_,steps)=>{
                    let mut bytes=arena(steps);for step in steps{tick()?;match step{optional_chaining::Step::Call{args,..}=>bytes=bytes.saturating_add(arena(args)),optional_chaining::Step::Member{..}=>{}}}bytes
                }
                ExprKind::Object(members)=>arena(members),
                ExprKind::Array(elements)=>arena(elements),
                ExprKind::RegExp(_)|ExprKind::This|ExprKind::NewTarget|ExprKind::Unary(..)|ExprKind::Yield{..}|ExprKind::Binary(..)|ExprKind::Conditional(..)|ExprKind::Sequence(..)|ExprKind::Assign(..)|ExprKind::Update(..)|ExprKind::Member(..)|ExprKind::Function(_)|ExprKind::Class(_)|ExprKind::Pattern(_)=>0,
            };
            bytes=bytes.saturating_add(owned);
        }
        for statement in &self.stmts {
            tick()?;let owned=match &statement.kind {
                StmtKind::Block(body)=>arena(body),StmtKind::Declare(_,declarations)=>arena(declarations),StmtKind::Function(name,_)=>name.capacity(),
                StmtKind::Switch{cases,body,..}=>arena(cases).saturating_add(arena(body)),
                StmtKind::Empty|StmtKind::Expr(_)|StmtKind::If(..)|StmtKind::While(..)|StmtKind::DoWhile(..)|StmtKind::For(..)|StmtKind::ForIn(..)|StmtKind::ForOf(..)|StmtKind::Return(_)|StmtKind::Yield(_)|StmtKind::YieldFrom(_)|StmtKind::Throw(_)|StmtKind::Try(..)|StmtKind::Break(_)|StmtKind::Continue(_)=>0,
            };bytes=bytes.saturating_add(owned);
        }
        for template in &self.templates {
            tick()?;bytes=bytes.saturating_add(template.name.as_ref().map_or(0,String::capacity)).saturating_add(arena(&template.params)).saturating_add(arena(&template.body)).saturating_add(arena(&template.resumption_plan));
        }
        for pattern in &self.patterns {
            tick()?;let owned=match &pattern.kind{binding_patterns::Kind::Name(name)=>name.capacity(),binding_patterns::Kind::Array(elements,_)=>arena(elements),binding_patterns::Kind::Object(properties,_)=>arena(properties),binding_patterns::Kind::Target(_)=>0};bytes=bytes.saturating_add(owned);
        }
        for class in &self.classes {
            tick()?;bytes=bytes.saturating_add(class.name.as_ref().map_or(0,String::capacity)).saturating_add(arena(&class.members));
            for member in &class.members{tick()?;match &member.key{class_parser::MemberKey::Literal(name)=>bytes=bytes.saturating_add(name.capacity()),class_parser::MemberKey::Computed(_)=>{}}}
        }
        Ok(bytes)
    }
}

#[cfg(test)]
#[path="program_storage_tests.rs"]
mod tests;
