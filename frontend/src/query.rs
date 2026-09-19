use crate::models::{FilterOptions, ProgramOption};
use serde_json::{Map, Value};

pub fn encode_value(value: &str) -> String {
    value.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
        (b as char).to_string()
    } else { format!("%{b:02X}") }).collect()
}
fn decode(value: &str, query: bool) -> Option<String> {
    let b = value.as_bytes(); let mut out = Vec::new(); let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => { let hi = (*b.get(i+1)? as char).to_digit(16)?; let lo = (*b.get(i+2)? as char).to_digit(16)?; out.push((hi*16+lo) as u8); i += 3; },
            b'+' if query => { out.push(b' '); i += 1; },
            v => { out.push(v); i += 1; }
        }
    }
    String::from_utf8(out).ok()
}
fn parameters(search: &str) -> Vec<(String,String)> {
    search.trim_start_matches('?').split('&').filter_map(|pair| {
        let (key,value) = pair.split_once('=')?;
        Some((decode(key,true)?,decode(value,true)?))
    }).collect()
}
pub fn parameter(search: &str, name: &str) -> Option<String> {
    parameters(search).into_iter().find(|(k,v)|k==name&&!v.is_empty()).map(|(_,v)|v)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramTab { Plan, Electives, All }
impl ProgramTab { pub fn slug(self) -> &'static str { match self { Self::Plan => "plan", Self::Electives => "electives", Self::All => "modules" } } }
#[derive(Clone, Debug, PartialEq)]
pub enum Route { Catalog(FilterOptions), Course(String), Program(String, ProgramTab), NotFound }
pub fn module_url(id: &str) -> String { format!("/catalog/module/{}", encode_value(id)) }
#[allow(dead_code)]
pub fn course_url(id: &str) -> String { module_url(id) }
pub fn program_url(slug: &str, tab: ProgramTab) -> String { format!("/study-programm/{}/{}", encode_value(slug), tab.slug()) }

fn slugify(s: &str) -> String {
    let transliterated=s.to_lowercase().replace('ä',"ae").replace('ö',"oe").replace('ü',"ue").replace('ß',"ss");
    transliterated.split(|c:char|!c.is_alphanumeric()).filter(|s|!s.is_empty()).collect::<Vec<_>>().join("-")
}
fn base_slug(p: &ProgramOption) -> String {
    let degree=crate::components::study_program_selector::format_degree_short(p.degree.as_deref()).replace('.',"");
    let year=p.po_version.as_deref().unwrap_or("").split(|c:char|!c.is_ascii_digit()).find(|s|s.len()==4).unwrap_or("ohne-po");
    let mut base=format!("{}-{}-{}",slugify(&degree),slugify(&p.program_name),year).trim_start_matches('-').to_string();
    let d=p.degree.as_deref().unwrap_or("").to_lowercase();
    for (needle,label) in [("ausbildungsintegrierend","ausbildung"),("praxisintegrierend","praxis"),("fern","fernstudium"),("teilzeit","teilzeit"),("erweiterte","erweitert"),("verringerte","verkuerzt"),("doppelabschluss","doppelabschluss")] {
        if d.contains(needle) {base.push('-');base.push_str(label);}
    }
    base
}
pub fn program_slug(id: &str, programs: &[ProgramOption]) -> String {
    let Some(p)=programs.iter().find(|p|p.id==id) else {return id.into()};
    let base=base_slug(p);
    let collisions:Vec<_>=programs.iter().filter(|other|base_slug(other)==base).collect();
    if collisions.len()==1 {return base}
    let suffix=format!("{}-{}",slugify(p.po_version.as_deref().unwrap_or("")),slugify(p.degree.as_deref().unwrap_or("")));
    let candidate=format!("{base}-{suffix}");
    if collisions.iter().filter(|other|other.po_version==p.po_version&&other.degree==p.degree).count()==1 {return candidate}
    // Only indistinguishable catalog branches need a stable short discriminator.
    let hash=id.bytes().fold(2166136261u32,|h,b|(h^b as u32).wrapping_mul(16777619));
    format!("{candidate}-{hash:08x}")
}
pub fn resolve_program(token: &str, programs: &[ProgramOption]) -> String {
    if programs.iter().any(|p|p.id==token) {return token.into()}
    programs.iter().find(|p|program_slug(&p.id,programs)==token).map(|p|p.id.clone()).unwrap_or_else(||token.into())
}
pub fn resolve_route(route: Route, programs: &[ProgramOption]) -> Route {
    match route {
        Route::Program(token,tab)=>Route::Program(resolve_program(&token,programs),tab),
        Route::Catalog(mut f)=>{f.program_id=resolve_program(&f.program_id,programs);Route::Catalog(f)},
        other=>other
    }
}
fn public_key(key:&str)->String {match key {"query"=>"search".into(),"program_id"=>"program".into(),"po_version"=>"po".into(),_=>key.replace('_',"-")}}
fn internal_key(key:&str)->String {match key {"search"|"q"=>"query".into(),"program"=>"program_id".into(),"po"=>"po_version".into(),_=>key.replace('-',"_")}}
fn ui_only(key:&str)->bool {key.starts_with("acc_")||key.ends_with("accordion_open")}

pub fn catalog_url(filters: &FilterOptions) -> String {
    let data=serde_json::to_value(filters).unwrap_or_default();
    let defaults=serde_json::to_value(FilterOptions::default()).unwrap_or_default();
    let mut pairs=Vec::new();
    if let Some(fields)=data.as_object(){for (key,value) in fields {
        if defaults.get(key)==Some(value)||ui_only(key){continue}
        let key=public_key(key);
        let values=match value {Value::Array(a)=>a.clone(),v=>vec![v.clone()]};
        for value in values {let text=match value{Value::String(s)=>s,v=>v.to_string()};pairs.push(format!("{}={}",key,encode_value(&text)));}
    }}
    if pairs.is_empty(){"/catalog".into()}else{format!("/catalog?{}",pairs.join("&"))}
}
fn catalog_filters(search:&str)->FilterOptions {
    let mut data:Map<String,Value>=parameter(search,"data").and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
    let defaults=serde_json::to_value(FilterOptions::default()).unwrap_or_default();
    let mut arrays=std::collections::HashSet::new();
    for (key,text) in parameters(search) {
        let key=internal_key(&key);
        let Some(kind)=defaults.get(&key) else{continue};
        if ui_only(&key){continue}
        let value=match kind {
            Value::String(_)=>Some(Value::String(text)),
            Value::Bool(_)=>match text.as_str(){"true"|"1"=>Some(Value::Bool(true)),"false"|"0"=>Some(Value::Bool(false)),_=>None},
            Value::Number(_)=>text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number),
            Value::Null if key=="semester"=>text.parse::<i64>().ok().filter(|n|*n>=0&&*n<=30).map(|n|Value::Number(n.into())),
            Value::Array(_)=>{
                if arrays.insert(key.clone()){data.insert(key.clone(),Value::Array(vec![]));}
                if let Some(Value::Array(a))=data.get_mut(&key){a.push(Value::String(text));} None
            },
            _=>None
        };
        if let Some(value)=value{data.insert(key,value);}
    }
    // Ignore malformed individual fields without losing other valid filters.
    let mut valid=Map::new();
    for (key,value) in data {let mut single=Map::new();single.insert(key.clone(),value.clone());if serde_json::from_value::<FilterOptions>(Value::Object(single)).is_ok(){valid.insert(key,value);}}
    let mut filters:FilterOptions=serde_json::from_value(Value::Object(valid)).unwrap_or_default();
    for (value,allowed,default) in [
        (&mut filters.duration,&["alle","1","2"][..],"alle"),
        (&mut filters.grading,&["alle","benotet","unbenotet"][..],"alle"),
        (&mut filters.module_type,&["alle","pflicht","wahlpflicht","fues"][..],"alle"),
        (&mut filters.limitation,&["ja","nein","nur"][..],"ja"),
        (&mut filters.fues,&["inkl","exkl","nur"][..],"inkl"),
        (&mut filters.sort_by,&["id","title","ects","events"][..],"title"),
    ] {if !allowed.contains(&value.as_str()){*value=default.into();}}
    if filters.min_credits<0. || filters.max_credits<filters.min_credits {filters.min_credits=0.;filters.max_credits=30.;}
    filters
}
pub fn parse_route(path: &str, search: &str) -> Route {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() || ["catalog", "catalogue", "catalouge"].contains(&trimmed) {
        if let Some(id)=parameter(search,"module"){return Route::Course(id)}
        if trimmed.is_empty() {if let Some(id)=parameter(search,"program"){return Route::Program(id,ProgramTab::Plan)}}
        return Route::Catalog(catalog_filters(search));
    }
    let parts:Vec<_>=trimmed.split('/').collect();
    match parts.as_slice(){
        ["catalog", "module", id] if !id.is_empty()=>decode(id,false).map(Route::Course).unwrap_or(Route::NotFound),
        ["course", id] if !id.is_empty()=>decode(id,false).map(Route::Course).unwrap_or(Route::NotFound),
        ["study-programm",id,tab] if !id.is_empty()=>{
            let tab=match *tab{"plan"=>ProgramTab::Plan,"electives"=>ProgramTab::Electives,"modules"=>ProgramTab::All,_=>return Route::NotFound};
            decode(id,false).map(|id|Route::Program(id,tab)).unwrap_or(Route::NotFound)
        },
        ["study-programm",id] if !id.is_empty()=>decode(id,false).map(|id|Route::Program(id,ProgramTab::Plan)).unwrap_or(Route::NotFound),
        _=>Route::NotFound
    }
}

#[cfg(test)]mod tests{
    use super::*;
    fn p(id:&str,name:&str,degree:&str,po:&str)->ProgramOption{ProgramOption{id:id.into(),program_name:name.into(),degree:Some(degree.into()),po_version:Some(po.into())}}
    #[test]fn slugs_resolve_uniquely(){let programs=vec![p("i","Informatik","Bachelor (universitär)","2008 - 2. SÄ 2024"),p("m","Informatik","Master (universitär)","2008"),p("d","Informatik","Bachelor (universitär) - Duales Studium, praxisintegrierend","2008")];assert_eq!(program_slug("i",&programs),"bsc-informatik-2008");for p in &programs{assert_eq!(resolve_program(&program_slug(&p.id,&programs),&programs),p.id)}assert_eq!(resolve_program("i",&programs),"i");}
    #[test]fn slug_collisions_remain_unambiguous(){let programs=vec![p("a","Größe","Bachelor","2024"),p("b","Größe","Bachelor","2024 - 1. SÄ 2025"),p("c","Größe","Bachelor","2024")];let slugs:std::collections::HashSet<_>=programs.iter().map(|p|program_slug(&p.id,&programs)).collect();assert_eq!(slugs.len(),3);for p in &programs{assert_eq!(resolve_program(&program_slug(&p.id,&programs),&programs),p.id)}}
    #[test]fn readable_filters_round_trip(){let f=FilterOptions{duration:"2".into(),grading:"benotet".into(),query:"Größe + Wärme & Strom".into(),program_id:"bsc-informatik-2008".into(),semester:Some(3),min_credits:2.5,lang_en:true,prof_includes:vec!["Müller & Co".into(),"Schmidt".into()],..Default::default()};let url=catalog_url(&f);assert!(url.starts_with("/catalog?"));assert!(url.contains("duration=2&grading=benotet"));assert!(!url.contains("data="));let(path,search)=url.split_once('?').unwrap();assert_eq!(parse_route(path,search),Route::Catalog(f));assert_eq!(catalog_url(&FilterOptions::default()),"/catalog");}
    #[test]fn invalid_fields_do_not_destroy_valid_filters(){let Route::Catalog(f)=parse_route("/catalog","?duration=2&grading=&semester=bad&lang-en=wrong&max-credits=NaN")else{panic!()};assert_eq!(f.duration,"2");assert_eq!(f.semester,None);assert_eq!(f.max_credits,30.);assert_eq!(f.grading,"alle");}
    #[test]fn unicode_and_reserved_ids_round_trip(){for id in ["SÄ_2024","A+B & C=1/%"]{assert_eq!(parse_route(&module_url(id),""),Route::Course(id.into()));assert_eq!(parse_route(&course_url(id),""),Route::Course(id.into()));assert_eq!(parse_route(&program_url(id,ProgramTab::Plan),""),Route::Program(id.into(),ProgramTab::Plan));}assert_eq!(parse_route("/catalog/module/A+B",""),Route::Course("A+B".into()));assert_eq!(parse_route("/course/A+B",""),Route::Course("A+B".into()));}
    #[test]fn invalid_routes(){for path in ["/catalog/module/%ZZ","/catalog/module/%","/catalog/module/%FF","/catalog/module/","/course/%ZZ","/course/%","/course/%FF","/course/","/study-programm/id/unknown","/unknown"]{assert_eq!(parse_route(path,""),Route::NotFound)}}
    #[test]fn legacy_links(){assert_eq!(parse_route("/","?module=11101"),Route::Course("11101".into()));assert_eq!(parse_route("/","?program=S%C3%84"),Route::Program("SÄ".into(),ProgramTab::Plan));let Route::Catalog(f)=parse_route("/catalouge","?data=%7B%22duration%22%3A%222%22%7D")else{panic!()};assert_eq!(f.duration,"2");let Route::Catalog(f2)=parse_route("/catalogue","?duration=1")else{panic!()};assert_eq!(f2.duration,"1");assert_eq!(parse_route("/course/11101",""),Route::Course("11101".into()));}
}
