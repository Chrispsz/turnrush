#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Gerador do roster v2 do TurnRush (Task 8-b) — IP original, 4 facções x 4 roles.

Contrato (engine v0.3, loader implementado por outro agente):
  {"version":2,"characters":[{"id","name","faction","role","hp","attack","defense","speed",
    "mystery":{name,chakra,cooldown,targets,damage_pct,applies_state,effect},
    "standard":{name,damage_pct,applies_state},
    "chase":{name,trigger,damage_pct,applies_state,max_per_round},
    "passive":{name,effect}}]}

Interpretação contratada (documentada em NOTAS-DESIGN.md):
  - em mysteries com targets ally_single, damage_pct = % do ATTACK aplicado como
    CURA (support) ou ESCUDO (guardian); applies_state só tem efeito em alvo inimigo.
  - enemies_front = inimigos da linha de frente; enemies_all = todos; random_2 = 2 aleatórios.
  - estados: launch (arremesso) | knockback (empurrão) | knockdown (derrubada) | none.
    No Naruto Online os estados foram normalizados (MECANICAS.md); o arremesso leve
    (小浮空) do dataset original colapsa para "launch" neste contrato.

Roda: python3 _gerador_roster.py  -> valida orçamento + cadeias e escreve roster-v2.json
"""
import json, os, sys

SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "roster-v2.json")

# ---------------------------------------------------------------------------
# 16 personagens. Assinatura mecânica de cada um herdada dos padrões do
# Naruto Online documentados em ../naruto-online/MECANICAS.md (ver notas).
# ---------------------------------------------------------------------------
CHARS = [
 # ============================ EMBER — Agressão/pressão (queima) ============
 dict(id="ignia", name="Ígnia Brasa-Viva", faction="ember", role="striker",
      hp=920, attack=112, defense=38, speed=62,
      mystery=dict(name="Detonação Solar", chakra=20, cooldown=3, targets="enemies_front",
                   damage_pct=120, applies_state="launch",
                   effect="Golpe devastador que arremessa o alvo e o deixa em Queimadura (dano contínuo por 2 rodadas)."),
      standard=dict(name="Corte em Brasa", damage_pct=70, applies_state="none"),
      chase=dict(name="Chute Vulcânico", trigger="launch", damage_pct=30, applies_state="knockdown", max_per_round=1),
      passive=dict(name="Sede de Brasa", effect="+15% de dano contra alvos em Queimadura.")),
 dict(id="orson", name="Orson Muralha-Fundida", faction="ember", role="guardian",
      hp=1140, attack=96, defense=58, speed=42,
      mystery=dict(name="Escudo de Obsidiana", chakra=20, cooldown=2, targets="ally_single",
                   damage_pct=90, applies_state="none",
                   effect="Concede escudo de 90% do ataque ao aliado e provoca os inimigos da frente."),
      standard=dict(name="Bastão de Lava", damage_pct=60, applies_state="knockdown"),
      chase=dict(name="Contragolpe Fundido", trigger="launch", damage_pct=25, applies_state="knockback", max_per_round=1),
      passive=dict(name="Guardião Incandescente", effect="-15% de dano recebido enquanto estiver na linha de frente.")),
 dict(id="amara", name="Amara Chama-Doce", faction="ember", role="support",
      hp=980, attack=100, defense=44, speed=55,
      mystery=dict(name="Fogueira Interior", chakra=15, cooldown=2, targets="ally_single",
                   damage_pct=100, applies_state="none",
                   effect="Cura 100% do ataque e gera +10 de chakra para a equipe."),
      standard=dict(name="Toque Morno", damage_pct=60, applies_state="launch"),
      chase=dict(name="Golpe Fervedor", trigger="knockdown", damage_pct=20, applies_state="none", max_per_round=1),
      passive=dict(name="Cinzas Que Aquecem", effect="No fim da rodada, cura 5% do HP do aliado com menos HP.")),
 dict(id="nimbo", name="Nimbo Véu-de-Fumaça", faction="ember", role="controller",
      hp=900, attack=104, defense=40, speed=58,
      mystery=dict(name="Névoa Sonífera", chakra=25, cooldown=3, targets="enemies_front",
                   damage_pct=85, applies_state="knockdown",
                   effect="Dano nos inimigos da frente e Derrubada (a fila de ação deles é adiada)."),
      standard=dict(name="Cajado Fumegante", damage_pct=60, applies_state="none"),
      chase=dict(name="Lança-Fumaça", trigger="knockback", damage_pct=25, applies_state="knockdown", max_per_round=1),
      passive=dict(name="Olhos de Cinza", effect="A primeira perseguição da equipe em cada rodada causa +10% de dano.")),
 # ============================ TIDE — Controle/sustentação (água) ===========
 dict(id="nereo", name="Nereo Lâmina-de-Sal", faction="tide", role="striker",
      hp=950, attack=110, defense=40, speed=60,
      mystery=dict(name="Corte da Maré Alta", chakra=20, cooldown=3, targets="enemies_front",
                   damage_pct=115, applies_state="launch",
                   effect="Corte pesado que arremessa o alvo e ignora 50% do escudo dele."),
      standard=dict(name="Golpe de Sal", damage_pct=70, applies_state="none"),
      chase=dict(name="Reflexo da Maré", trigger="launch", damage_pct=30, applies_state="knockback", max_per_round=1),
      passive=dict(name="Lâmina Salmoura", effect="+10% de dano; ataques contra alvos com escudo causam +25%.")),
 dict(id="dalva", name="Dalva Recife", faction="tide", role="guardian",
      hp=1150, attack=94, defense=60, speed=40,
      mystery=dict(name="Barreira de Coral", chakra=20, cooldown=2, targets="ally_single",
                   damage_pct=90, applies_state="none",
                   effect="Escudo de 90% do ataque no aliado alvo; reflete 20% do dano que ele absorver."),
      standard=dict(name="Cajado de Coral", damage_pct=60, applies_state="knockdown"),
      chase=dict(name="Recife Vivo", trigger="launch", damage_pct=25, applies_state="knockback", max_per_round=1),
      passive=dict(name="Pele de Coral", effect="-15% de dano recebido na linha de frente.")),
 dict(id="mel", name="Mel Maré-Mãe", faction="tide", role="support",
      hp=1000, attack=98, defense=46, speed=52,
      mystery=dict(name="Maré Curativa", chakra=15, cooldown=2, targets="ally_single",
                   damage_pct=100, applies_state="none",
                   effect="Cura 100% do ataque e dissipa 1 estado negativo do alvo."),
      standard=dict(name="Espuma Firme", damage_pct=60, applies_state="launch"),
      chase=dict(name="Contracorrente", trigger="knockdown", damage_pct=20, applies_state="none", max_per_round=1),
      passive=dict(name="Onda de Chakra", effect="No início da rodada, gera +8 de chakra para a equipe.")),
 dict(id="ligeia", name="Ligeia Névoa-Fria", faction="tide", role="controller",
      hp=890, attack=102, defense=38, speed=57,
      mystery=dict(name="Prisão de Vapor", chakra=25, cooldown=3, targets="enemies_front",
                   damage_pct=85, applies_state="knockdown",
                   effect="Dano e Derrubada nos inimigos da frente (prisão de vapor)."),
      standard=dict(name="Vapor Cortante", damage_pct=60, applies_state="none"),
      chase=dict(name="Redemoinho", trigger="knockback", damage_pct=25, applies_state="knockdown", max_per_round=1),
      passive=dict(name="Neblina Densa", effect="Inimigos Derrubados recebem +15% de dano dos ataques da equipe.")),
 # ============================ VERDANT — Desgaste/crescimento (terra) =======
 dict(id="tina", name="Tina Espinho-Gêmeo", faction="verdant", role="striker",
      hp=930, attack=108, defense=40, speed=58,
      mystery=dict(name="Rajada de Espinhos", chakra=25, cooldown=3, targets="random_2",
                   damage_pct=85, applies_state="launch",
                   effect="85% de dano em 2 inimigos aleatórios, arremessando-os; aplica Veneno (dano contínuo por 2 rodadas)."),
      standard=dict(name="Chicote de Cipó", damage_pct=70, applies_state="none"),
      chase=dict(name="Duplo Espinho", trigger="launch", damage_pct=30, applies_state="knockback", max_per_round=1),
      passive=dict(name="Peçonha Persistente", effect="Os ataques de Tina aplicam Veneno (dano contínuo por 2 rodadas).")),
 dict(id="aro", name="Aro Cascão", faction="verdant", role="guardian",
      hp=1150, attack=92, defense=62, speed=38,
      mystery=dict(name="Casco de Pedra-Viva", chakra=25, cooldown=3, targets="self",
                   damage_pct=100, applies_state="none",
                   effect="Escudo de 100% do ataque que ganha +25% de valor ao fim de cada rodada enquanto existir."),
      standard=dict(name="Casco Esmagador", damage_pct=60, applies_state="knockdown"),
      chase=dict(name="Garras de Raiz", trigger="launch", damage_pct=25, applies_state="knockback", max_per_round=1),
      passive=dict(name="Casca Dura", effect="-15% de dano recebido; reflete 10% do dano de ataque padrão recebido.")),
 dict(id="yara", name="Yara Seiva-Viva", faction="verdant", role="support",
      hp=1020, attack=96, defense=46, speed=50,
      mystery=dict(name="Seiva Vital", chakra=15, cooldown=2, targets="ally_single",
                   damage_pct=100, applies_state="none",
                   effect="Cura 100% do ataque; o alvo regenera 6% do HP máximo por rodada durante 2 rodadas."),
      standard=dict(name="Folha Cortante", damage_pct=60, applies_state="launch"),
      chase=dict(name="Raízes Ágeis", trigger="knockdown", damage_pct=20, applies_state="none", max_per_round=1),
      passive=dict(name="Broto Renascido", effect="No fim da rodada, cura 5% do HP do aliado com menos HP.")),
 dict(id="ivo", name="Ívo Corda-Fina", faction="verdant", role="controller",
      hp=880, attack=100, defense=40, speed=54,
      mystery=dict(name="Marionete Eremita", chakra=25, cooldown=3, targets="enemies_front",
                   damage_pct=80, applies_state="knockdown",
                   effect="A marionete investe: dano e Derrubada nos inimigos da frente."),
      standard=dict(name="Fio de Aço", damage_pct=60, applies_state="none"),
      chase=dict(name="Contra-Peso", trigger="knockback", damage_pct=25, applies_state="knockdown", max_per_round=1),
      passive=dict(name="Corda-Fina", effect="Inimigos com estado aplicado pela equipe causam -10% de dano.")),
 # ============================ STORM — Velocidade/combo (raio/vento) ========
 dict(id="reia", name="Reia Passo-Trovão", faction="storm", role="striker",
      hp=930, attack=115, defense=36, speed=70,
      mystery=dict(name="Impacto do Trovão", chakra=30, cooldown=3, targets="enemies_front",
                   damage_pct=130, applies_state="launch",
                   effect="Dano devastador que arremessa o alvo e ignora 30% da defesa dele."),
      standard=dict(name="Golpe Trovejante", damage_pct=75, applies_state="launch"),
      chase=dict(name="Fenda do Raio", trigger="launch", damage_pct=35, applies_state="knockback", max_per_round=1),
      passive=dict(name="Iniciativa Trovejante", effect="Age primeiro na rodada de abertura; +10% de velocidade.")),
 dict(id="aki", name="Aki Contra-Corte", faction="storm", role="guardian",
      hp=1100, attack=95, defense=56, speed=46,
      mystery=dict(name="Manto do Redemoinho", chakra=20, cooldown=2, targets="self",
                   damage_pct=80, applies_state="none",
                   effect="Escudo de 80% do ataque em si; enquanto durar, contra-ataca quem a atingir."),
      standard=dict(name="Lâmina Espelhada", damage_pct=60, applies_state="knockdown"),
      chase=dict(name="Corte Reverso", trigger="launch", damage_pct=25, applies_state="knockback", max_per_round=1),
      passive=dict(name="Espelho de Vento", effect="Ao ser atingida por ataque padrão, contra-ataca com 40% do ataque (1x por rodada).")),
 dict(id="alisio", name="Alísio Sopro-Ligeiro", faction="storm", role="support",
      hp=980, attack=97, defense=44, speed=60,
      mystery=dict(name="Brisa Guiadora", chakra=15, cooldown=2, targets="ally_single",
                   damage_pct=90, applies_state="none",
                   effect="Cura 90% do ataque e concede Aceleração (o alvo age antes dos inimigos na próxima rodada)."),
      standard=dict(name="Rajada Firme", damage_pct=60, applies_state="launch"),
      chase=dict(name="Vento de Cauda", trigger="knockdown", damage_pct=20, applies_state="none", max_per_round=1),
      passive=dict(name="Ritmo de Batalha", effect="A primeira perseguição da equipe por rodada gera +5 de chakra.")),
 dict(id="volta", name="Volta Ciclone", faction="storm", role="controller",
      hp=900, attack=103, defense=38, speed=59,
      mystery=dict(name="Tempestade Circular", chakra=30, cooldown=4, targets="enemies_all",
                   damage_pct=80, applies_state="launch",
                   effect="80% de dano em todos os inimigos, arremessando-os."),
      standard=dict(name="Discos de Vento", damage_pct=60, applies_state="none"),
      chase=dict(name="Descarga de Acompanhamento", trigger="knockback", damage_pct=25, applies_state="knockdown", max_per_round=1),
      passive=dict(name="Carga Estática", effect="Inimigos Arremessados perdem 5 de velocidade até o fim da rodada.")),
]

FACOES = {"ember", "tide", "verdant", "storm"}
ROLES = {"striker", "guardian", "support", "controller"}
TARGETS = {"enemies_front", "enemies_all", "ally_single", "self", "random_2"}
ESTADOS = {"launch", "knockback", "knockdown", "none"}

def validar(chars):
    erros = []
    vistos = set()
    for c in chars:
        tag = c["id"]
        if tag in vistos: erros.append(f"id duplicado {tag}")
        vistos.add(tag)
        if not c["id"].replace("-", "").isalnum() or " " in c["id"]:
            erros.append(f"{tag}: id não kebab-case")
        if c["faction"] not in FACOES: erros.append(f"{tag}: facção inválida")
        if c["role"] not in ROLES: erros.append(f"{tag}: role inválida")
        if not (850 <= c["hp"] <= 1150): erros.append(f"{tag}: hp {c['hp']} fora de 850–1150")
        if not (60 <= c["standard"]["damage_pct"] <= 80): erros.append(f"{tag}: standard fora de 60–80")
        if not (80 <= c["mystery"]["damage_pct"] <= 130): erros.append(f"{tag}: mystery fora de 80–130")
        if not (15 <= c["mystery"]["chakra"] <= 40): erros.append(f"{tag}: chakra fora de 15–40")
        if not (2 <= c["mystery"]["cooldown"] <= 4): erros.append(f"{tag}: cooldown fora de 2–4")
        if not (20 <= c["chase"]["damage_pct"] <= 35): erros.append(f"{tag}: chase fora de 20–35")
        if c["chase"]["max_per_round"] != 1: erros.append(f"{tag}: max_per_round != 1")
        if c["mystery"]["targets"] not in TARGETS: erros.append(f"{tag}: targets inválido")
        for s in (c["mystery"]["applies_state"], c["standard"]["applies_state"], c["chase"]["applies_state"], c["chase"]["trigger"]):
            if s not in ESTADOS: erros.append(f"{tag}: estado inválido {s}")
    # cobertura: 1 de cada role por facção
    for f in FACOES:
        roles = [c["role"] for c in chars if c["faction"] == f]
        if sorted(roles) != sorted(ROLES): erros.append(f"{f}: roles incompletas {roles}")
    # cadeias: por facção, >=2 pares launch→chase funcionais e >=1 knockdown→chase
    for f in FACOES:
        time = [c for c in chars if c["faction"] == f]
        fontes_launch, fontes_kd, chasers_launch, chasers_kd = set(), set(), set(), set()
        for c in time:
            if c["mystery"]["applies_state"] == "launch": fontes_launch.add(c["id"])
            if c["standard"]["applies_state"] == "launch": fontes_launch.add(c["id"] + " (padrão)")
            if c["mystery"]["applies_state"] == "knockdown": fontes_kd.add(c["id"])
            if c["standard"]["applies_state"] == "knockdown": fontes_kd.add(c["id"] + " (padrão)")
            if c["chase"]["trigger"] == "launch": chasers_launch.add(c["id"])
            if c["chase"]["trigger"] == "knockdown": chasers_kd.add(c["id"])
        pares_l = len(fontes_launch) * len(chasers_launch)
        pares_kd = len(fontes_kd) * len(chasers_kd)
        if pares_l < 2: erros.append(f"{f}: só {pares_l} pares launch→chase")
        if pares_kd < 1: erros.append(f"{f}: nenhum par knockdown→chase")
        print(f"  {f}: pares launch→chase={pares_l} (fontes {len(fontes_launch)} x caçadores {len(chasers_launch)}), knockdown→chase={pares_kd}")
    return erros

def main():
    print("Validando cadeias por facção:")
    erros = validar(CHARS)
    for f in FACOES:
        somas = {
            "hp": sum(c["hp"] for c in CHARS if c["faction"] == f),
            "attack": sum(c["attack"] for c in CHARS if c["faction"] == f),
            "speed": sum(c["speed"] for c in CHARS if c["faction"] == f),
            "chase": sum(c["chase"]["damage_pct"] for c in CHARS if c["faction"] == f),
        }
        print(f"  {f}: hp={somas['hp']} attack={somas['attack']} speed={somas['speed']} chase_soma={somas['chase']}")
    if erros:
        print("ERROS:"); [print(" -", e) for e in erros]; sys.exit(1)
    out = {"version": 2, "characters": CHARS}
    with open(SAIDA, "w", encoding="utf-8") as fh:
        json.dump(out, fh, ensure_ascii=False, indent=2)
    print(f"OK -> {SAIDA} | personagens: {len(CHARS)}")

if __name__ == "__main__":
    main()
