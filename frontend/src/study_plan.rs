use std::collections::BTreeMap;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlanCell {
    pub id: String,
    pub table: String,
    pub page: usize,
    pub row: String,
    pub semesters: Vec<i64>,
    pub min: f64,
    pub max: f64,
    #[serde(default)] pub workload: Vec<f64>,
    #[serde(default)] pub credit_semester: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StudyPlan {
    #[serde(default)] pub cells: Vec<PlanCell>,
    #[serde(default)] pub totals: Vec<PlanCell>,
    #[serde(default)] pub plan_names: BTreeMap<String,String>,
}

pub fn number(n:f64)->String {
    if n.fract().abs()<0.001 {format!("{n:.0}")} else {format!("{n:.1}").replace('.',",")}
}

pub fn semester_label(start:i64,end:i64)->String {
    if start>0 && end>start {format!("{start}.–{end}. Semester")}
    else if start>0 {format!("{start}. Semester")}
    else {"Semester nicht festgelegt".into()}
}

impl PlanCell {
    pub fn credits(&self)->String {
        if (self.max-self.min).abs()>0.001 {format!("{}–{} ECTS",number(self.min),number(self.max))}
        else {format!("{} ECTS",number(self.min))}
    }
    pub fn period(&self)->String {semester_label(*self.semesters.first().unwrap_or(&0),*self.semesters.last().unwrap_or(&0))}
    pub fn is_workload(&self)->bool {self.semesters.len()>1&&self.workload.len()==self.semesters.len()&&self.credit_semester>0}
    pub fn is_window(&self)->bool {self.semesters.len()>1&&!self.is_workload()}
    pub fn workload_in(&self,sem:i64)->Option<f64>{self.semesters.iter().position(|s|*s==sem).and_then(|i|self.workload.get(i).copied())}
    pub fn is_total(&self)->bool {matches!(self.row.trim().to_lowercase().as_str(),"summe"|"summe studium"|"summe gesamt"|"summe erreichte lp"|"gesamt"|"insgesamt"|"total"|"total credits")}
}

impl StudyPlan {
    pub fn variants(&self)->BTreeMap<String,String>{
        self.cells.iter().map(|c|(c.table.clone(),self.plan_names.get(&c.table).cloned().unwrap_or_else(||format!("Studienplan · Seite {}",c.page)))).collect()
    }
    pub fn totals_for(&self,table:&str)->Vec<&PlanCell>{self.totals.iter().filter(|c|c.table==table&&c.is_total()).collect()}
    pub fn total_label(&self)->String {
        let totals:Vec<f64>=self.variants().keys().filter_map(|table|{
            let t=self.totals_for(table);if t.is_empty(){None}else{Some(t.iter().map(|c|c.min).sum())}
        }).collect();
        if let Some(first)=totals.first(){if totals.iter().all(|x|(x-first).abs()<0.01){return format!("{} ECTS",number(*first))}}
        "Je Studienvariante".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workload_is_not_a_flexible_window(){
        let c=PlanCell{semesters:vec![2,3],workload:vec![3.,3.],credit_semester:3,min:6.,max:6.,..Default::default()};
        assert!(c.is_workload());assert!(!c.is_window());assert_eq!(c.workload_in(2),Some(3.));assert_eq!(c.credits(),"6 ECTS");assert_eq!(c.period(),"2.–3. Semester");
    }
    #[test]
    fn range_is_never_split_or_rounded(){
        let c=PlanCell{semesters:vec![5,6],min:10.,max:24.,..Default::default()};
        assert!(c.is_window());assert_eq!(c.workload_in(5),None);assert_eq!(c.credits(),"10–24 ECTS");
        assert_eq!(number(2.5),"2,5");
    }
    #[test]
    fn variants_and_workload_totals_are_not_added(){
        let mut p=StudyPlan::default();for table in ["a","b"]{p.cells.push(PlanCell{table:table.into(),..Default::default()});p.totals.push(PlanCell{table:table.into(),row:"Summe".into(),min:180.,max:180.,..Default::default()});p.totals.push(PlanCell{table:table.into(),row:"Summe Aufwand".into(),min:180.,max:180.,..Default::default()});}
        assert_eq!(p.total_label(),"180 ECTS");
    }
}
