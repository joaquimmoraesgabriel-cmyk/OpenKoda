//! One bounded, deterministic match on the user's own chat message.
//! Sources, prior turns and assistant content cannot trigger a skill.
use serde_json::Value;

fn words(text: &str) -> String {
    let mut result = String::from(" ");
    for ch in text.to_lowercase().chars() {
        let ch = match ch {
            'á'|'à'|'ã'|'â'=>'a','é'|'ê'=>'e','í'=>'i','ó'|'õ'|'ô'=>'o','ú'|'ü'=>'u','ç'=>'c',other=>other,
        };
        if ch.is_alphanumeric(){result.push(ch);} else if !result.ends_with(' '){result.push(' ');}
    }
    if !result.ends_with(' '){result.push(' ');}
    result
}

pub(super) fn match_id(message:&str, catalog:&Value)->Option<String>{
    let text=words(message);
    let rows=catalog["skills"].as_array()?;
    let mut best=0;
    let mut selected=None;
    for row in rows.iter().take(500).filter(|r|r["invocation"]=="automatic_or_explicit"){
        let Some(id)=row["id"].as_str() else{continue};
        let triggers:&[&str]=match id {
            "code-review"=>&["review this diff","review this code","revisar o diff","revise o diff","revise este codigo","revisao de codigo"],
            "codebase-design"=>&["deep module","module design","design do modulo","redesenhar a interface do modulo"],
            "diagnosing-bugs"=>&["debug this","diagnose this","diagnostique","diagnosticar o bug","depurar","investigue a regressao"],
            "domain-modeling"=>&["modelagem de dominio","domain model","glossario de dominio"],
            "pr"=>&["pull request description","pr description","descricao do pr","corpo do pull request"],
            "prototype"=>&["build a prototype","crie um prototipo","faca um prototipo"],
            "research"=>&["research primary sources","pesquise fontes primarias","pesquisa com fontes primarias"],
            "tdd"=>&["tdd","test driven development","desenvolvimento guiado por testes","teste primeiro"],
            "wizard"=>&["bash wizard","assistente interativo em bash"],
            "git-guardrails-claude-code"=>&["claude code git guardrails","bloquear comandos git no claude code"],
            "migrate-to-shoehorn"=>&["migrate to shoehorn","migrar para shoehorn"],
            "scaffold-exercises"=>&["scaffold exercises","gerar estrutura de exercicios"],
            "setup-pre-commit"=>&["configurar pre commit","setup pre commit"],
            "grilling"=>&["grill me","questione meu plano","questione o meu plano"],
            "writing-for-agents"=>&["write agents md","escreva o agents md","redigir o agents md"],
            _=>&[],
        };
        let score=triggers.iter().filter(|phrase|text.contains(&format!(" {phrase} ")))
            .map(|phrase|phrase.split_whitespace().count()).max().unwrap_or(0);
        if score>best {best=score;selected=Some(id.to_owned());}
        else if score>0 && score==best {selected=None;}
    }
    selected
}

#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]fn automatic_chat_skills_only_use_unique_eligible_triggers(){
        let skills=json!({"skills":[{"id":"tdd","invocation":"explicit"},{"id":"code-review","invocation":"automatic_or_explicit"},
            {"id":"diagnosing-bugs","invocation":"automatic_or_explicit"}]});
        assert_eq!(match_id("Faça uma revisão de código.",&skills).as_deref(),Some("code-review"));
        assert!(match_id("TDD com testes",&skills).is_none());
        assert!(match_id("debug this depois de revisar o repositório",&skills).is_some());
        assert!(match_id("depurar e TDD",&json!({"skills":[{"id":"tdd","invocation":"automatic_or_explicit"},{"id":"diagnosing-bugs","invocation":"automatic_or_explicit"}]})).is_none());
        assert!(match_id("mostre o resumo",&skills).is_none());
    }
}
