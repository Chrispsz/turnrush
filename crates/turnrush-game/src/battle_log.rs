//! Parser do log textual de batalha 4v4 → eventos estruturados para o
//! playback visual. **Tolerante por design**: qualquer linha que não case
//! com um formato conhecido vira [`EvKind::Other`] e ainda assim aparece no
//! log da interface — o texto do engine é a fonte da verdade, o visual só
//! enriquece o que entende.

/// Tipo de evento visual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvKind {
    /// Início de rodada (abertura).
    Round,
    /// Unidade conjurou a Mystery.
    Cast,
    /// Ataque padrão/chain: origem → alvo sofre dano.
    Attack,
    /// Dano em alvo (follow-up de cadeia, DoT, etc.).
    Damage,
    /// Unidade caiu.
    Death,
    /// Cura.
    Heal,
    /// Reflexo: alvo da linha sofre dano de volta.
    Reflect,
    /// Estado aplicado (Arremessado, Queimadura, …) com ou sem dano.
    State,
    /// Elo de cadeia: perseguição (chase) de um aliado.
    Chase,
    /// Chakra de equipe.
    Chakra,
    /// Fim de batalha.
    End,
    /// Linha não reconhecida (sempre exibida no log).
    Other,
}

/// Evento estruturado do playback.
#[derive(Clone, Debug)]
pub struct ParsedEvent {
    pub kind: EvKind,
    /// Quem executa a ação (atacante/conjurador/refletidor sofredor).
    pub source: Option<String>,
    /// Quem sofre (dano/cura/estado/morte).
    pub target: Option<String>,
    /// Dano/cura/chakra quando aplicável.
    pub amount: i64,
    /// Linha original (exibida no log da UI).
    pub text: String,
}

impl ParsedEvent {
    fn other(line: &str) -> Self {
        ParsedEvent { kind: EvKind::Other, source: None, target: None, amount: 0, text: line.to_string() }
    }
}

/// Remove espaços iniciais e marcadores de destaque.
fn clean(line: &str) -> &str {
    line.trim().trim_start_matches('─').trim_start().trim_end_matches('─').trim()
}

/// Extrai um número inteiro logo após um padrão (ex.: "sofre 120").
fn num_after(s: &str, pat: &str) -> Option<i64> {
    let idx = s.find(pat)? + pat.len();
    let rest = &s[idx..];
    let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Tag do Mystery no log do engine, montada em runtime (o literal `[m`
/// sofre sanitização ANSI em algumas ferramentas de escrita de arquivo).
#[cfg(test)]
#[allow(dead_code)]
pub fn tag_mystery() -> String {
    format!("[{}ystery]", 'm')
}

/// Remove um prefixo de tag `[...]` de um nome extraído do log.
fn strip_tag(s: Option<String>) -> Option<String> {
    s.map(|mut n| {
        if n.starts_with('[') {
            if let Some(end) = n.find(']') {
                n = n[end + 1..].trim_start().to_string();
            }
        }
        n
    })
}

/// Nome antes de um delimitador: "X usa", "X sofre", "cura em X".
fn name_before(s: &str, pat: &str) -> Option<String> {
    let idx = s.find(pat)?;
    let head = s[..idx].trim();
    if head.is_empty() {
        None
    } else {
        Some(head.to_string())
    }
}

/// Nome depois de um padrão até o fim/separator "·"/"(".
fn name_after(s: &str, pat: &str) -> Option<String> {
    let idx = s.find(pat)? + pat.len();
    let rest = s[idx..].trim();
    let end = rest.find(['(', '·', ',']).unwrap_or(rest.len());
    let n = rest[..end].trim();
    if n.is_empty() { None } else { Some(n.to_string()) }
}

/// Converte UMA linha do log em [`ParsedEvent`].
pub fn parse_line(line: &str) -> ParsedEvent {
    let c = clean(line);
    let has = |p: &str| c.contains(p);

    if has("rodada") && has("abre") {
        let n = num_after(c, "rodada ").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Round, source: None, target: None, amount: n, text: line.trim().to_string() };
    }
    if has("batalha 4v4") {
        return ParsedEvent { kind: EvKind::Other, source: None, target: None, amount: 0, text: line.trim().to_string() };
    }
    if has("persegue") {
        // "[chase] X persegue {estado} de Y → D de dano"
        let source = strip_tag(name_before(c, " persegue"));
        let seg = c.split("persegue").nth(1).unwrap_or("");
        let target = seg
            .split(" de ")
            .nth(1)
            .map(|rest| rest.split("→").next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty());
        let dmg = num_after(c, "→ ").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Chase, source, target, amount: dmg, text: line.trim().to_string() };
    }
    if has("fim:") {
        return ParsedEvent { kind: EvKind::End, source: None, target: None, amount: 0, text: line.trim().to_string() };
    }
    if has("cai") && has("✖") {
        let target = name_before(c, " cai")
            .map(|s| s.trim_start_matches('\u{2716}').trim().to_string())
            .or_else(|| name_after(c, "✖ "));
        return ParsedEvent { kind: EvKind::Death, source: None, target, amount: 0, text: line.trim().to_string() };
    }
    if has("reflexo:") {
        let seg = c.split("reflexo:").nth(1).unwrap_or("");
        let target = name_before(seg, " sofre");
        let dmg = num_after(seg, "sofre ").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Reflect, source: None, target, amount: dmg, text: line.trim().to_string() };
    }
    if has("usa") && has("«") {
        let source = strip_tag(name_before(c, " usa "));
        let kind = if has("[mystery]") { EvKind::Cast } else { EvKind::Attack };
        // "[padrão] U usa «S» → T sofre D"
        let (target, amount) = if has("→") {
            let t = c.split('→').nth(1).unwrap_or("");
            let tgt = name_before(t, " sofre ");
            let dmg = num_after(t, "sofre ").unwrap_or(0);
            (tgt, dmg)
        } else {
            (None, 0)
        };
        return ParsedEvent { kind, source, target, amount, text: line.trim().to_string() };
    }
    if has("sofre") && has("e recebe") {
        // "T sofre D e recebe Estado"
        let target = name_before(c, " sofre ");
        let dmg = num_after(c, "sofre ").unwrap_or(0);
        let _st = name_after(c, "e recebe ").map(|s| s.trim().to_string());
        return ParsedEvent { kind: EvKind::State, source: None, target, amount: dmg, text: line.trim().to_string() };
    }
    if has("sofre") {
        let target = name_before(c, " sofre ");
        let dmg = num_after(c, "sofre ").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Damage, source: None, target, amount: dmg, text: line.trim().to_string() };
    }
    if has("cura") && has(" em ") {
        let target = name_after(c, " em ");
        let heal = num_after(c, "cura ").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Heal, source: None, target, amount: heal, text: line.trim().to_string() };
    }
    if has("escudo") && has(" em ") {
        let target = name_after(c, " em ");
        return ParsedEvent { kind: EvKind::State, source: None, target, amount: 0, text: line.trim().to_string() };
    }
    if has("recebe") && !has("ganha") {
        let target = name_before(c, " recebe ");
        return ParsedEvent { kind: EvKind::State, source: None, target, amount: 0, text: line.trim().to_string() };
    }
    if has("chakra") {
        let amount = num_after(c, "+").unwrap_or(0);
        return ParsedEvent { kind: EvKind::Chakra, source: None, target: None, amount, text: line.trim().to_string() };
    }
    ParsedEvent::other(line.trim())
}

/// Converte o log completo.
pub fn parse_all(lines: &[String]) -> Vec<ParsedEvent> {
    lines.iter().map(|l| parse_line(l)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_formatos_do_engine() {
        let evs = parse_all(&[
            "── batalha 4v4 ── A: [Golem] vs B: [Bruto]".into(),
            "── rodada 1 (abre: A) ──".into(),
            "  [mystery] Golem usa «Muralha» (chakra -30)".into(),
            "    equipe ganha +10 de chakra".into(),
            "  [padrão] Golem usa «Punho» → Bruto sofre 120".into(),
            "    Bruto sofre 60 e recebe Arremessado".into(),
            "    reflexo: Golem sofre 15".into(),
            "    cura 30 em Golem".into(),
            "    escudo 20 em Golem".into(),
            "    ✖ Bruto caiu".into(),
            "── fim: TeamAWin em 3 rodadas · A=80% HP · B=0% HP · cadeias=5 · mysteries=2 ──".into(),
        ]);

        assert_eq!(evs[0].kind, EvKind::Other);
        assert_eq!(evs[1].kind, EvKind::Round);
        assert_eq!(evs[1].amount, 1);

        assert_eq!(evs[2].kind, EvKind::Cast);
        assert_eq!(evs[2].source.as_deref(), Some("Golem"));

        assert_eq!(evs[3].kind, EvKind::Chakra);
        assert_eq!(evs[3].amount, 10);

        assert_eq!(evs[4].kind, EvKind::Attack);
        assert_eq!(evs[4].source.as_deref(), Some("Golem"));
        assert_eq!(evs[4].target.as_deref(), Some("Bruto"));
        assert_eq!(evs[4].amount, 120);

        assert_eq!(evs[5].kind, EvKind::State);
        assert_eq!(evs[5].target.as_deref(), Some("Bruto"));
        assert_eq!(evs[5].amount, 60);

        assert_eq!(evs[6].kind, EvKind::Reflect);
        assert_eq!(evs[6].target.as_deref(), Some("Golem"));
        assert_eq!(evs[6].amount, 15);

        assert_eq!(evs[7].kind, EvKind::Heal);
        assert_eq!(evs[7].target.as_deref(), Some("Golem"));
        assert_eq!(evs[7].amount, 30);

        assert_eq!(evs[8].kind, EvKind::State);
        assert_eq!(evs[8].target.as_deref(), Some("Golem"));

        assert_eq!(evs[9].kind, EvKind::Death);
        assert_eq!(evs[9].target.as_deref(), Some("Bruto"));

        assert_eq!(evs[10].kind, EvKind::End);
    }

    #[test]
    fn linha_desconhecida_nao_perde_texto() {
        let ev = parse_line("    algo novo do engine 42");
        assert_eq!(ev.kind, EvKind::Other);
        assert_eq!(ev.text, "algo novo do engine 42");
    }

    #[test]
    fn cadeia_padrao_com_target_e_dano() {
        let ev = parse_line("  [padrão] Sombra usa «Lâmina Sombria» → Monge sofre 95");
        assert_eq!(ev.kind, EvKind::Attack);
        assert_eq!(ev.source.as_deref(), Some("Sombra"));
        assert_eq!(ev.target.as_deref(), Some("Monge"));
        assert_eq!(ev.amount, 95);
    }
}
