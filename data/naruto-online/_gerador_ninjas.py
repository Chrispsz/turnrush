#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Gerador do dataset curado de 40 ninjas do Naruto Online (TurnRush — Task 8-b).

Fonte: /tmp/dominus/research/fresh_ninja_list_clean.json (255 ninjas estruturados).
Curadoria: seleção manual de 8 ninjas icônicos por elemento (5 elementos x 8 = 40),
tradução manual das descrições para pt-BR e normalização dos estados chineses.

Estados normalizados (pt-BR):
  小浮空 = arremesso_leve   大浮空 = arremesso   击退 = empurrao   击倒 = derrubada
  (gatos de perseguição por contagem de combo/condição usam "nenhum" e explicam no texto)

Saída: ninjas.json neste diretório. Rode: python3 _gerador_ninjas.py
"""
import json, re, os, sys, collections

FONTE = "/tmp/dominus/research/fresh_ninja_list_clean.json"
SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ninjas.json")

PROP = {"101": "Água", "102": "Fogo", "103": "Vento", "104": "Raio", "105": "Terra"}

# ---------------------------------------------------------------------------
# Curadoria: 40 ninjas icônicos (id_no = ingame_id do dataset original).
# Campos: nome pt-BR + 5 habilidades traduzidas; gatilho/aplícado são derivados
# automaticamente do texto chinês de skill_3, com overrides manuais quando o
# gatilho não é um estado (combo/condição).
# ---------------------------------------------------------------------------
C = {
 # ----------------------------- VENTO (103) -----------------------------
 "11001101": dict(nome="Naruto Uzumaki [Jovem]", grade="B",
  my=("Massa de Sombras Clonadas", "Causa 90% de dano a 2–5 inimigos do esquadrão e invoca 1–2 clones com 5% de vida. Como Golpe Final: invoca 5 clones."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Chute Ascendente", "Persegue Derrubada: causa 25% de dano e aplica Arremesso Leve."),
  pas=("Técnica Sexy", "No início da batalha, aplica Fraqueza (-30% de dano) nos ninjas homens do esquadrão inimigo."),
  cha=("Ira", "Quando um ninja de Konoha do seu time é derrotado, Naruto ganha +30% de taijutsu e defesa.")),
 "11013101": dict(nome="Gaara [Jovem]", grade="S",
  my=("Túmulo das Cataratas de Areia", "Causa 160% de dano a 5–7 inimigos do campo; o alvo escolhido sofre Derrubada e Imobilização. Como Golpe Final: dano +20%."),
  ap=("Chuva de Areia", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Caixão Prisioneiro de Areia", "Persegue Arremesso: causa 25% de dano, aplica Arremesso Leve e Imobilização."),
  pas=("Guarda de Areia", "No início da batalha, perde 20% da vida máxima e ganha escudo igual a 150% da resistência."),
  cha=("Escudo de Areia", "Com escudo ativo, é imune a estados negativos e a gatilhos de perseguição.")),
 "11014101": dict(nome="Temari [Jovem]", grade="A",
  my=("Ninjutsu de Vento: Bomba-Ciclone", "Causa 70% de dano a 3 inimigos do esquadrão; o alvo escolhido sofre Empurrão. Como Golpe Final: dano +30% e +6 alvos."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Ninjutsu de Vento: Rajada", "Persegue Arremesso Leve: causa 25% de dano e aplica Arremesso."),
  pas=("Dança do Leque de Três Estrelas", "Após usar a Oitiva, o cooldown dela cai para 1 e Temari ganha +40% de ninjutsu; no máximo 2 vezes por batalha."),
  cha=("Ataque de Cobertura", "Com escudo ativo, +30% de taxa de crítico.")),
 "11015101": dict(nome="Kankurô [Jovem]", grade="A",
  my=("Marionete: Corvo", "Invoca a marionete Corvo (60% de vida) na frente de um aliado e concede a ele escudo de 40% da resistência. Como Golpe Final: escudo em dobro."),
  ap=("Ataque de Shuriken", "Causa 40% de dano ao inimigo à frente; chance de causar Veneno."),
  per=("Marionete: Formiga Negra", "Persegue Derrubada: causa 25% de dano, aplica Arremesso Leve e Veneno."),
  pas=("Ataque de Marionete", "Com a marionete em campo, o ataque padrão passa a ser feito por ela: 100% de dano com Arremesso Leve e Veneno garantidos."),
  cha=("Marionete Reserva", "Após usar a Oitiva, o cooldown dela cai para 1; no máximo 1 vez por batalha.")),
 "11044101": dict(nome="Minato Namikaze [Quarto Hokage]", grade="SS",
  my=("Voo do Deus do Trovão: Segunda Etapa", "Causa 80% de dano a 7–9 inimigos do campo, aplica Marca e interrompe Oitivas; o alvo escolhido sofre Arremesso. Como Golpe Final: dano +30%."),
  ap=("Ninjutsu de Vento: Rasengan", "Causa 60% de dano ao inimigo à frente; alta chance de causar Empurrão."),
  per=("Corte do Deus do Trovão", "Persegue Empurrão: causa 20% de dano, aplica Derrubada e Marca; até 2 vezes por rodada."),
  pas=("Clarão Dourado", "Cada 1% a mais de iniciativa sobre o alvo aumenta o dano de Minato em 1% (até +45%)."),
  cha=("Técnica do Deus do Trovão", "Chance altíssima de esquivar de Oitiva, ataque padrão ou perseguição; no máximo 1 vez por rodada.")),
 "11021101": dict(nome="Orochimaru", grade="SS",
  my=("Espada Kusanagi", "Causa 120% de dano, Arremesso e interrupção de Oitiva no alvo escolhido; +2 no cooldown da Oitiva dele. Como Golpe Final: dano +30%."),
  ap=("Arte das Serpentes", "Causa 60% de dano ao inimigo à frente; alta chance de causar Arremesso."),
  per=("Mãos de Serpentes Ocultas", "Persegue Arremesso: causa 25% de dano, aplica Derrubada e Paralisia."),
  pas=("Ambição", "Quando qualquer ninja morre em campo, o cooldown da Oitiva de Orochimaru cai em 1 e ele recupera 8% da vida máxima."),
  cha=("Concessão do Selo Maldito", "No início da batalha, marca 1 ninja de grade B ou A do time: +10% de ninjutsu ao sofrer dano e +5 pontos de Golpe Final.")),
 "11061101": dict(nome="Konan", grade="S",
  my=("Dança das Folhas de Papel: Punição", "Causa 100% de dano a até 5 inimigos em cruz no esquadrão do alvo; o alvo escolhido tem a Oitiva interrompida. Como Golpe Final: dano +30%."),
  ap=("Nevasca de Papel", "Causa 70% de dano ao inimigo à frente; chance de causar Derrubada."),
  per=("Grou de Papel", "Persegue Arremesso Leve: causa 20% de dano e aplica Empurrão; até 2 vezes por rodada."),
  pas=("Clone de Papel", "Chance altíssima de esquivar de perseguições; no máximo 1 vez por rodada."),
  cha=("Anjo de Deus", "Enquanto viva: Oitivas de ninjas Akatsuki do time custam -20 de chakra e, antes da primeira ação de cada rodada, ninjas Akatsuki do campo ganham +8% de taijutsu e ninjutsu.")),
 "11039101": dict(nome="Asuma Sarutobi", grade="S",
  my=("Genjutsu: Sakura Tripla", "Causa 95% de dano a 2 inimigos do esquadrão; o alvo escolhido sofre Derrubada. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Empurrão."),
  per=("Ninjutsu de Fogo: Cinzas Ardentes", "Persegue Empurrão: causa 25% de dano, aplica Derrubada e Queimadura."),
  pas=("Lâmina de Chakra", "No fim da rodada, consome o chakra restante do time: a cada 20 de chakra consumido, Asuma ganha +50% de ninjutsu."),
  cha=("Forças Guardadas", "Asuma recebe +100% de defesa e resistência até usar a Oitiva pela primeira vez na batalha.")),
 # ----------------------------- FOGO (102) -----------------------------
 "11031101": dict(nome="Itachi Uchiha", grade="SS",
  my=("Tsukuyomi", "Pode ser usada gastando vida: causa 80% de dano ao alvo escolhido com Imobilização e Colapso Mental; depois a Oitiva vira Amaterasu. Como Golpe Final: sempre acerta."),
  ap=("Técnica de Shuriken", "Ataque-surpresa: causa 70% de dano ao inimigo à frente; chance de causar Empurrão e invoca 1 clone (30% de vida) com Genjutsu Sharingan e Clone de Corvos."),
  per=("Ninjutsu de Fogo: Grande Dragão de Fogo", "Dispara quando a equipe encadeia 3 ou mais perseguições na rodada: causa 20% de dano a 3 inimigos do esquadrão e aplica Queimadura."),
  pas=("Clone de Corvos", "A cada rodada, chance altíssima de esquivar de 1 Oitiva."),
  cha=("Genjutsu: Espelho do Céu e da Terra", "O primeiro estado negativo que Itachi sofrer em cada rodada é refletido ao autor.")),
 "11022101": dict(nome="Jiraiya", grade="SS",
  my=("Ninjutsu de Fogo: Chama do Óleo de Sapo", "Causa 100% de dano e aplica Queimadura a 6 inimigos do campo; o alvo escolhido sofre Derrubada e Redução de Velocidade. Como Golpe Final: dano +30%."),
  ap=("Invocação: Sapo", "Ataca 4 inimigos do esquadrão causando 40% de dano; chance de causar Derrubada."),
  per=("Cabelo Leão Desgrenhado", "Persegue Empurrão: causa 20% de dano, aplica Arremesso Leve e Marca; até 2 vezes por rodada."),
  pas=("O Sábio Galante", "Quando uma mulher do time usa a Oitiva, o custo em chakra da Oitiva de Jiraiya cai em 20 (valor volta ao normal após o uso)."),
  cha=("Ninjutsu de Terra: Lodo Lento", "No início da batalha, aplica Redução de Velocidade em 2 inimigos aleatórios do esquadrão.")),
 "11023101": dict(nome="Tsunade", grade="SS",
  my=("Arte da Regeneração", "Cura o aliado escolhido em 40% do ninjutsu de Tsunade e o torna imune a estados negativos por 2 rodadas. Como Golpe Final: cura +30%."),
  ap=("Ataque de Força Sobre-humana", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Paraíso Puro: Punho Celeste", "Persegue Arremesso Leve: causa 25% de dano, aplica Arremesso e acerta em crítico."),
  pas=("Apostadora", "No início da batalha, ganha 1 destes bônus: 1 ataque padrão extra por rodada, +50% de taxa de crítico ou +30% de cura."),
  cha=("Neurotoxina", "+10% de taxa de crítico; os críticos de Tsunade aplicam Confusão.")),
 "11024101": dict(nome="Hiruzen Sarutobi [Terceiro Hokage]", grade="SS",
  my=("Ninjutsu de Terra: Projétil-Dragão de Terra", "Causa 100% de dano a 3 inimigos do esquadrão; o alvo escolhido sofre Derrubada e Imobilização. Como Golpe Final: Imobilização em 3 alvos."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Pedras do Ermitão", "Persegue Arremesso: causa 25% de dano, aplica Derrubada e dá ao macaco Enma escudo de 30% da resistência."),
  pas=("Invocação: Macaco Enma", "No início da batalha, invoca Enma (70% de vida), que faz 2 ataques padrão por rodada com alta chance de Arremesso e Marca."),
  cha=("Selamento: Rei dos Mortos", "Com menos de 20% de vida, a Oitiva é substituída: causa 999% de dano ao alvo escolhido e a si mesmo, +9 no próprio cooldown e sem ressurreição. Como Golpe Final: dano +30%.")),
 "11008201": dict(nome="Shikamaru Nara [Shippuden]", grade="S",
  my=("Arte de Imitação de Sombras", "Aplica Imobilização no alvo escolhido; se ele já estiver imobilizado, usa Aglomeração de Sombras em vez disso. Como Golpe Final: Aglomeração de Sombras + Imobilização."),
  ap=("Recuperação de Chakra", "Em vez de atacar: recupera 20 de chakra e dissipa 1 estado negativo."),
  per=("Espinho de Sombra", "Persegue Arremesso Leve: causa 20% de dano, aplica Derrubada e Imobilização."),
  pas=("Peão do Shogi", "Ninjas do time que NÃO estejam na mesma linha ou coluna que Shikamaru recebem +20% de taxa de combo e dano."),
  cha=("Armadilha", "Shikamaru causa +40% de dano a ninjas fora de Konoha; inimigos derrotados pela Oitiva dele não ressuscitam.")),
 "11041101": dict(nome="Kurenai Yûhi", grade="S",
  my=("Genjutsu: Sakura Tripla", "Causa 80% de dano a 3 inimigos do esquadrão e aplica Arremesso Leve no alvo escolhido; em seguida dá a todo o time escudo de 20% da resistência. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 60% de dano ao inimigo à frente; chance de causar Empurrão."),
  per=("Genjutsu: Névoa", "Persegue Derrubada: causa 20% de dano e aplica Empurrão; até 2 vezes por rodada."),
  pas=("Genjutsu: Espelho Refletor", "O primeiro estado negativo sofrido pelo time em cada rodada é refletido a um inimigo aleatório."),
  cha=("Absorção de Chakra", "Quando o time inimigo usa a primeira Oitiva da rodada, Kurenai ganha chakra igual ao custo dela.")),
 "11009201": dict(nome="Chôji Akimichi [Shippuden]", grade="A",
  my=("Super Empurrão", "Causa 70% de dano a 9 inimigos do campo; o alvo escolhido sofre Derrubada. Como Golpe Final: dano +30%."),
  ap=("Punho de Multiplicação", "Causa 50% de dano ao inimigo à frente; chance de Derrubada. Ganha +1 alvo por rodada (até 3)."),
  per=("Carro de Guerra Humano", "Persegue Derrubada: causa 25% de dano e aplica Empurrão."),
  pas=("Super Recuperação", "Quando recebe cura, ganha +15% de taijutsu e 1 ataque padrão extra; até 2 vezes por rodada."),
  cha=("Arte de Multiplicação", "+25% de defesa para Chôji por cada ninja atrás dele na mesma linha.")),
 "11040101": dict(nome="Anko Mitarashi", grade="A",
  my=("Ninjutsu de Fogo: Fogos Desordenados", "Causa 70% de dano e aplica Queimadura aos inimigos de uma linha inteira; o alvo escolhido sofre Arremesso. Como Golpe Final: reabre a barreira de fogo."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Mãos de Serpentes Ocultas", "Persegue Arremesso: causa 25% de dano, aplica Derrubada e Paralisia."),
  pas=("Barreira de Fogo: Chamas da Luz", "No início da batalha, abre uma barreira por 2 rodadas: ninjas de Fogo do seu campo causam +30% de dano."),
  cha=("Sem Hesitação", "A primeira Oitiva individual por rodada mirada em um aliado do time tem o alvo trocado para Anko; no máximo 1 vez por rodada.")),
 # ----------------------------- ÁGUA (101) -----------------------------
 "11032101": dict(nome="Kisame Hoshigaki", grade="S",
  my=("Ninjutsu de Água: Grande Projétil-Tubarão", "Causa 80% de dano a 4 inimigos do esquadrão; o alvo escolhido sofre Derrubada. Como Golpe Final: +30% de taxa de crítico."),
  ap=("Ataque da Samehada", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Talho Pesado", "Persegue Empurrão: causa 25% de dano e aplica Arremesso Leve."),
  pas=("A Besta sem Cauda", "+1% de taijutsu por ponto de chakra que o time tiver; com chakra sobrando, é imune a estados negativos."),
  cha=("Devorador de Chakra", "Ninjas que sofreram taijutsu de Kisame pagam +20 de chakra nas Oitivas; quando eles usam Oitiva, Kisame ganha 20 de chakra.")),
 "11017101": dict(nome="Zabuza Momochi", grade="S",
  my=("Ninjutsu de Água: Névoa Ocultadora", "Aplica Cegueira em 3 inimigos do campo e dissipa 1 efeito positivo de cada um. Como Golpe Final: +9 alvos."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Ninjutsu de Água: Dragão de Água", "Persegue Arremesso Leve: causa 20% de dano e aplica Empurrão; até 2 vezes por rodada."),
  pas=("Invencível", "Quando Zabuza derrota alguém no ataque padrão, ganha 1 ataque padrão extra e recupera 5% da vida máxima."),
  cha=("Demônio da Névoa", "Zabuza faz 2 ataques padrão por rodada.")),
 "11018101": dict(nome="Haku", grade="A",
  my=("Arte Secreta: Cristais de Gelo Espelhados", "Causa 70% de dano a 3–6 inimigos do esquadrão; o alvo escolhido sofre Arremesso Leve. Como Golpe Final: reabre a barreira de gelo."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Ninjutsu de Gelo: Mil Agulhas de Água", "Persegue Empurrão: causa 25% de dano e aplica Arremesso Leve."),
  pas=("Barreira Secreta de Gelo", "No início da batalha, abre uma barreira por 2 rodadas: ninjas de Água e Vento do seu campo causam +30% de dano."),
  cha=("Compaixão", "Haku causa +30% de dano, porém nunca derrota o inimigo (deixa-o com 1 de vida).")),
 "11069101": dict(nome="Mei Terumî", grade="SS",
  my=("Kekkei Genkai: Névoa Fervente", "Causa 70% de dano (Água+Fogo) a 5–8 inimigos do campo, dissipa 1 efeito positivo e aplica Dissolução; o alvo escolhido tem a Oitiva interrompida. Como Golpe Final: reabre a barreira."),
  ap=("Ninjutsu de Água: Chicote d'Água", "Ataque-surpresa: causa 70% de dano ao inimigo à frente com +15% de crítico acumulado; chance de causar Empurrão e Seladura de Chakra."),
  per=("Ninjutsu de Água: Três Dragões de Água", "Persegue Arremesso Leve ou Empurrão: causa 25% de dano e aplica Empurrão; recupera vida igual a 15% do ninjutsu; até 2 vezes por rodada."),
  pas=("Névoa Habilidosa", "No início da batalha, dissipa 1 efeito positivo inimigo e abre uma barreira por 2 rodadas: inimigos perdem 15% de resistência a Fogo/Água e os ataques padrão deles têm 50% de chance de mirar em alvos aleatórios."),
  cha=("Casamento?!", "Ao dar ou sofrer crítico (inclusive em curas), dissipa todos os estados negativos próprios e ganha 50 pontos de Golpe Final.")),
 "11056101": dict(nome="Sai", grade="A",
  my=("Superarte de Tinta: Leão Dançante", "Causa 80% de dano a 6 inimigos do esquadrão; o alvo escolhido sofre Arremesso Leve. Como Golpe Final: +3 alvos."),
  ap=("Superarte de Tinta", "Golpes em sequência no inimigo à frente: 50%, 100% e 150% de dano; chance de Empurrão e, alternando, Marca, dissipação de 1 efeito positivo ou Imobilização."),
  per=("Superarte de Tinta: Serpente", "Persegue Derrubada: causa 25% de dano e aplica Arremesso Leve."),
  pas=("Talento do Pintor", "Cada invocação que entra no time inimigo reduz em 1 o cooldown da Oitiva de Sai."),
  cha=("Missão Confidencial", "Sai prioriza o ninja da posição 1 inimiga e causa +10% de dano a ele.")),
 "11052101": dict(nome="Suigetsu Hôzuki", grade="S",
  my=("Canhão d'Água Duplo", "Causa 110% de dano ao alvo escolhido e interrompe a Oitiva dele. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Ninjutsu de Água: Dragão de Água", "Persegue Arremesso Leve: causa 20% de dano e aplica Empurrão; até 2 vezes por rodada."),
  pas=("Arte de Liquificação", "Recebe -30% de dano físico comum, mas +30% de dano de Raio."),
  cha=("Espada Quebrada", "No início de cada rodada, ganha 1 ataque padrão extra; ao derrotar alguém no ataque padrão, ganha mais 1 ataque extra e +30% de taijutsu.")),
 "11020101": dict(nome="Kabuto Yakushi", grade="A",
  my=("Bisturi de Chakra: Corte", "Causa 100% de dano a 3 inimigos do esquadrão; o alvo escolhido sofre Empurrão. Como Golpe Final: dano +30%."),
  ap=("Arte do Eremita das Palmas", "Em vez de atacar: cura o aliado mais ferido do time em 20–30% do ninjutsu e dissipa todos os estados negativos dele."),
  per=("Bisturi de Chakra", "Persegue Derrubada: causa 25% de dano e aplica Arremesso."),
  pas=("Exploração de Fraquezas", "Kabuto causa +30% de dano a ninjas de grade B e aplica Paralisia ao acertar ninjas de grade S ou SS."),
  cha=("Espião", "Oitures e curas inimigas podem escolher Kabuto como alvo válido (ele conta como alvo legítimo para efeitos de suporte inimigos).")),
 "11006101": dict(nome="Tenten [Jovem]", grade="B",
  my=("Armas Invocadas: Chuva de Lâminas", "Causa 65% de dano a 6–8 inimigos do campo. Como Golpe Final: dano +40%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Derrubada."),
  per=("Dois Dragões Ascendentes", "Dispara quando a equipe acumula 10 golpes combinados: causa 20% de dano a 2 inimigos do esquadrão."),
  pas=("Reposição de Ferramentas", "No fim da rodada, a cada 20 de chakra que sobrar no time, o cooldown da Oitiva de Tenten cai em 1."),
  cha=("Na Mosca", "+30% de dano crítico de Tenten.")),
 # ----------------------------- RAIO (104) -----------------------------
 "11002101": dict(nome="Sasuke Uchiha [Jovem]", grade="A",
  my=("Ninjutsu de Fogo: Grande Bola de Fogo", "Causa 80% de dano a 6 inimigos do esquadrão; o alvo escolhido sofre Empurrão e Queimadura. Como Golpe Final: dano +20% e Queimadura em 6 alvos."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Chute Voraz", "Persegue Arremesso Leve: causa 25% de dano e aplica Empurrão."),
  pas=("Ninjutsu de Fogo: Fênix Flamejante", "Dispara quando a equipe acumula 10 golpes combinados: causa 20% de dano a 2 inimigos do esquadrão."),
  cha=("Golpe Preventivo", "No início da batalha, Sasuke ganha 100 pontos de Golpe Final.")),
 "11016101": dict(nome="Kakashi Hatake", grade="S",
  my=("Ninjutsu de Terra: Perseguição de Cachorro", "Causa 120% de dano ao alvo escolhido com Arremesso e Marca (+30% de dano recebido pelo alvo). Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso e Paralisia."),
  per=("Raikiri: Choque Duplo", "Persegue Arremesso: causa 25% de dano, aplica Derrubada e Paralisia."),
  pas=("Sharingan: Cópia", "Cada Oitiva usada por inimigos dá a Kakashi +40% de ninjutsu e taijutsu até o fim da rodada (até 3 acúmulos)."),
  cha=("O Maior Especialista de Konoha", "Kakashi conta como possuidor dos 5 elementos para bônus de equipe e sofre -10% de dano dos 5 elementos.")),
 "11012101": dict(nome="Hinata Hyûga [Jovem]", grade="B",
  my=("Guarda: Sessenta e Quatro Palmas do Círculo", "Eleva muito a taxa de esquiva do Círculo de Palmas (passiva) e o faz disparar até 3 vezes; dura até o fim da rodada. Como Golpe Final: ganha 2 ataques padrão extras."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Derrubada."),
  per=("Punho Gentil", "Persegue Empurrão: causa 25% de dano, aplica Derrubada e tem 20% de chance de Seladura de Chakra."),
  pas=("Círculo de Palmas Celeste", "20% de chance de esquivar de um ataque padrão e contra-atacar com 40% de dano; no máximo 1 vez por rodada."),
  cha=("Indomável", "Depois que o time usar 4 Oitivas, Hinata volta à luta com 20% da vida máxima caso tenha sido derrotada.")),
 "11065101": dict(nome="Pain [Tendo]", grade="SS",
  my=("Chibaku Tensei", "Causa 200% de dano a todo o campo inimigo; o alvo escolhido sofre Arremesso. Como Golpe Final: dano +30%."),
  ap=("Os Seis em Um", "Ataca 6 inimigos do campo causando 70% de dano e acumula 10 golpes combinados."),
  per=("Atração Celeste Universal", "Persegue Empurrão: causa 20% de dano e aplica Derrubada; até 2 vezes por rodada."),
  pas=("Shinra Tensei", "Chance altíssima de esquivar de Oitiva, ataque padrão ou perseguição e contra-atacar com 40% de dano; no máximo 1 vez por rodada."),
  cha=("Reencarnação dos Seis Caminhos", "Enquanto Pain estiver vivo, eleva muito a resistência a críticos de todo o time.")),
 "11075101": dict(nome="Killer Bee", grade="SS",
  my=("YO!", "Dá +30% de taxa de crítico a todos os ninjas do seu campo e reduz em 1 o gatilho das habilidades de Bijuu; não acumula. Como Golpe Final: ganha 3 ataques padrão extras."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Golpe de Força Bruta", "Persegue Arremesso Leve: causa 25% de dano, aplica Empurrão e Paralisia."),
  pas=("Bijuu: Oito Caudas", "Cada ataque padrão de Bee (ou cada habilidade de Bijuu usada pelo campo) dá +15% de taijutsu; após 8, libera a Oitiva Rugido da Oito Caudas."),
  cha=("Rugido da Oito Caudas", "Habilidade liberada pela passiva: causa 200% de dano a 8 inimigos do campo e aplica Empurrão em alvos aleatórios.")),
 "11070101": dict(nome="A [Quarto Raikage]", grade="SS",
  my=("Queda do Meteoro do Trovão", "Causa 150% de dano aos inimigos em cruz no esquadrão do alvo, aplicando Derrubada; se o alvo estiver com menos de 20% de vida, causa 10x de dano. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Ataca 1 inimigo aleatório causando 90% de dano; chance de causar Empurrão."),
  per=("Machado do Trovão Justo", "Persegue Derrubada: causa 25% de dano e aplica Arremesso Leve."),
  pas=("Modo Chakra de Raio", "Nas rodadas pares: ganha Corpo Rígido (imune a estados), -50% de dano recebido e grande aumento no efeito de Oitivas, ataques e perseguições, por 1 rodada."),
  cha=("Físico de Ferro", "Imune a estados negativos (exceto Seladura de Chakra); recupera 5% da vida máxima sempre que um aliado faz ataque padrão.")),
 "11073101": dict(nome="Darui", grade="S",
  my=("Ninjutsu de Raio: Onda Negra", "Causa 150% de dano a 4 inimigos do esquadrão; o alvo escolhido sofre Empurrão e Seladura de Chakra. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso Leve."),
  per=("Estilo Tempestade: Circo de Lasers", "Persegue Arremesso Leve: causa 25% de dano e aplica Empurrão."),
  pas=("Estilo Tempestade Negro", "+50% de ataque de Água e Raio para o time; ninjas de Água ou Raio ganham 1 ataque padrão extra no início da rodada."),
  cha=("Descontraído", "Com a Oitiva em cooldown: +60% de defesa e resistência; com a Oitiva disponível: +30% de taijutsu.")),
 "11025101": dict(nome="Kimimaro Kaguya", grade="S",
  my=("Dança das Samambaias", "Gasta 10% da vida máxima e causa 95% de dano a 7 inimigos do campo; o alvo escolhido sofre Arremesso e Imobilização. Como Golpe Final: dano +40%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Dança do Salgueiro", "Persegue Arremesso: causa 20% de dano e aplica Empurrão; até 2 vezes por rodada."),
  pas=("Dança do Lariço", "Chance altíssima de esquivar de ataques padrão; no máximo 1 vez por rodada."),
  cha=("Selo Maldito", "Cada vez que sofre dano, ganha +10% em um atributo (ninjutsu/taijutsu/defesa/resistência) e 5 pontos de Golpe Final.")),
 # ----------------------------- TERRA (105) -----------------------------
 "11007101": dict(nome="Ino Yamanaka [Jovem]", grade="B",
  my=("Arte de Perturbação da Mente", "Causa 20% de dano ao alvo escolhido e aplica Confusão (o alvo ataca os próprios aliados). Como Golpe Final: ganha 1 ataque padrão extra."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Derrubada."),
  per=("Dança das Pétalas Caindo", "Persegue Arremesso Leve: causa 25% de dano e aplica Empurrão."),
  pas=("Arte de Cura", "Antes da 1ª ação de cada rodada, cura o homem com menos vida do time em 15% do ninjutsu e dissipa 1 estado negativo dele."),
  cha=("Arte da Comunicação Mental", "Enquanto Ino estiver viva, inimigos em Confusão recebem +10% de dano.")),
 "11010101": dict(nome="Kiba Inuzuka [Jovem]", grade="A",
  my=("Presas Perfurantes", "Causa 85% de dano ao alvo escolhido aplicando Empurrão (+3 alvos com o Clone Bestial em campo). Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Derrubada."),
  per=("Presas Racheadas", "Dispara quando a equipe acumula 10 golpes combinados: causa 20% de dano a 2 inimigos do esquadrão."),
  pas=("Clone Bestial", "No início da batalha, invoca 1 clone (50% de vida); enquanto ele existir, todo o time recebe +30% de taxa de combo."),
  cha=("Companheiro Precioso", "A primeira Oitiva individual por rodada mirada no Clone Bestial tem o alvo trocado para Kiba; no máximo 1 vez por rodada.")),
 "11011101": dict(nome="Shino Aburame [Jovem]", grade="A",
  my=("Arte Secreta: Enxame de Insetos", "Causa 80% de dano a 5–9 inimigos do campo; o alvo escolhido sofre Arremesso Leve. Como Golpe Final: dano +30%."),
  ap=("Insetos de Punho", "Causa 70% de dano ao inimigo à frente; chance de causar Empurrão."),
  per=("Enxame: Cacho", "Persegue Derrubada: causa 25% de dano e aplica Arremesso Leve."),
  pas=("Insetos Destruidores", "Se o chakra do esquadrão do alvo estiver zerado, Shino causa +5% de dano e aplica Veneno."),
  cha=("Armadura de Insetos", "+20% de defesa para Shino; quem acerta taijutsu nele sofre um pequeno dano de retorno.")),
 "11005101": dict(nome="Neji Hyûga [Jovem]", grade="A",
  my=("Sessenta e Quatro Palmas do Círculo", "Causa 130% de dano ao alvo escolhido, aplica Empurrão, Seladura de Chakra (não pode usar Oitiva) e 16 golpes combinados. Como Golpe Final: dano +30%."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Palmas do Círculo Vazio: Quebra-Céu", "Persegue Derrubada: causa 25% de dano e aplica Arremesso."),
  pas=("Círculo de Palmas Celeste", "60% de chance de esquivar de um ataque padrão e contra-atacar com 40% de dano; no máximo 1 vez por rodada."),
  cha=("Destino dos Fracos", "Os ataques de Neji aplicam Seladura de Chakra em ninjas de grade B.")),
 "11004101": dict(nome="Rock Lee [Jovem]", grade="B",
  my=("Oito Portões Internos", "Gasta 20% da vida e ganha 2 ataques padrão extras. Como Golpe Final: +1 ataque extra."),
  ap=("Ataque de Taijutsu", "Causa 70% de dano ao inimigo à frente; chance de causar Arremesso."),
  per=("Lótus Oculta", "Persegue Arremesso: causa 25% de dano e aplica Derrubada; até 2 vezes por rodada."),
  pas=("Determinação", "+0,4% de taxa de crítico para cada 1% de vida que Lee tiver perdido."),
  cha=("Golpe Pesado", "Os críticos de Lee aplicam Imobilização (o alvo não age).")),
 "11050101": dict(nome="Chiyo", grade="S",
  my=("Arte de Marionetes", "Invoca aleatoriamente a marionete Pai ou Mãe (60% de vida) e dá ao alvo escolhido escudo de 50%. Como Golpe Final: invoca as duas marionetes."),
  ap=("Atuação: Corpo Afiado", "Em vez de atacar: dá 1 ataque padrão extra a 1–2 aliados aleatórios do time (exceto ela)."),
  per=("Lâmina de Ataque", "Persegue Arremesso Leve: causa 25% de dano e aplica Empurrão."),
  pas=("Atuação: Corpos e Mentes em Um", "Com Pai e Mãe vivos, o ataque padrão dela é substituído: 140% de dano a 3–5 inimigos do esquadrão, com Arremesso Leve garantido em 1 alvo."),
  cha=("Duelo de Marionetistas", "A cada marionete destruída no campo, Chiyo ganha +20% de taijutsu e ninjutsu.")),
 "11057101": dict(nome="Yamato", grade="S",
  my=("Técnica Hokage: Domínio da Árvore", "Causa 50% de dano a 4 inimigos do esquadrão e impede que recuperem chakra; o alvo escolhido sofre Arremesso e interrupção de Oitiva. Como Golpe Final: reabre a barreira."),
  ap=("Arte da Grande Floresta", "Causa 70% de dano ao inimigo à frente; chance de causar Empurrão."),
  per=("Ninjutsu de Madeira: Pilar Gigante", "Persegue Derrubada: causa 25% de dano e aplica Arremesso."),
  pas=("Ninjutsu de Madeira: Mar de Árvores", "No início da batalha, abre uma barreira por 2 rodadas: ninjas de Terra do seu campo causam +30% de dano."),
  cha=("Chakra de Madeira", "Antes da 1ª ação de cada rodada, recupera 20 de chakra e 10% da vida máxima.")),
 "11071101": dict(nome="Ônoki", grade="SS",
  my=("Estilo Pó: Prisão da Origem", "Causa 100% de dano (Vento+Fogo+Terra) a 3–5 inimigos do esquadrão e abre a Barreira de Pó por 1 rodada. Como Golpe Final: barreira dura +1 rodada."),
  ap=("Formação de Pedras", "Causa 70% de dano (Terra) ao inimigo à frente e reduz 10% da resistência dele; alta chance de causar Empurrão."),
  per=("Ninjutsu de Terra: Núcleo Sísmico", "Persegue Derrubada: causa 25% de dano e aplica Arremesso; até 2 vezes por rodada."),
  pas=("Ninjutsu de Terra: Rocha Leve", "Antes da 1ª ação de cada rodada, 3 ninjas de Vento/Fogo/Terra do time dissipam 1 estado negativo e ganham 1 ataque padrão extra."),
  cha=("Velho e Vigoroso", "Imune a estados negativos (exceto Imobilização); cada 1% de vida que Ônoki tiver dá +0,8% de defesa ao time.")),
}

# Overrides de gatilho/aplicação (quando o texto de skill_3 não segue o padrão
# 追打[estado] — gatilho por combo/condição ou gatilho duplo).
OVR = {
 "11031101": ("nenhum", "nenhum"),   # Itachi: dispara a partir da 3ª perseguição da rodada
 "11006101": ("nenhum", "nenhum"),   # Tenten: gatilho 10 golpes combinados
 "11002101": ("nenhum", "nenhum"),   # Sasuke: passiva-combo (10 golpes) — perseguição é a de jovem
 "11010101": ("nenhum", "nenhum"),   # Kiba: gatilho 10 golpes combinados
 "11069101": ("arremesso_leve", "empurrao"),  # Mei: gatilho duplo (Arremesso Leve OU Empurrão)
}

GAT = [("小浮空", "arremesso_leve"), ("大浮空", "arremesso"), ("击退", "empurrao"), ("倒地", "derrubada")]
ORDEM = ["arremesso_leve", "arremesso", "empurrao", "derrubada"]

def derivar_gatilho(txt):
    for zh, pt in GAT:
        if zh in txt:
            return pt
    return "nenhum"

def derivar_aplica(txt):
    # parte de efeito após 造成 (causa): primeiro estado da cadeia canônica presente
    m = re.search(r"造成[^。]*(小浮空|大浮空|击退|倒地)", txt)
    if m:
        alvo = m.group(1)
        return dict(GAT)[alvo]
    return "nenhum"

def main():
    dados = json.load(open(FONTE))
    por_ing = {}
    for x in dados:
        por_ing[(x.get("ingame_id") or "").strip()] = x

    faltando = [k for k in C if k not in por_ing]
    if faltando:
        sys.exit("ERRO: ids ausentes na fonte: %s" % faltando)

    ninjas, avisos = [], []
    for id_no in sorted(C, key=lambda k: int(por_ing[k]["property"].strip())):
        x = por_ing[id_no]
        cur = C[id_no]
        prop = x["property"].strip()
        if prop not in PROP:
            sys.exit("ERRO: property desconhecida %s em %s" % (prop, id_no))
        elemento = PROP[prop]
        grade = (x.get("game_grade") or "").strip()

        note = x.get("note1") or ""
        m = re.search(r"消耗查克拉：(\d+)", note)
        chakra = int(m.group(1)) if m else None
        m = re.search(r"冷却回合：(\d+)", note)
        cd = int(m.group(1)) if m else None
        if chakra is None or cd is None:
            sys.exit("ERRO: note1 incompleto em %s: %r" % (id_no, note))

        s3 = (x.get("skill_3_des") or "")
        if id_no in OVR:
            gatilho, aplica = OVR[id_no]
        else:
            gatilho = derivar_gatilho(s3)
            aplica = derivar_aplica(s3)
            if gatilho == "nenhum" and "追打" in s3:
                avisos.append("gatilho não padronizado em %s (%s)" % (id_no, s3.strip()[:40]))
        if aplica == "nenhum" and gatilho != "nenhum" and "nenhum" not in OVR.get(id_no, ("", "")):
            avisos.append("perseguição sem estado aplicado em %s" % id_no)

        ninjas.append({
            "id_no": id_no,
            "nome": cur["nome"],
            "grade": grade,
            "elemento": elemento,
            "mystery": {"nome": cur["my"][0], "descricao_pt": cur["my"][1], "chakra": chakra, "cooldown": cd},
            "ataque_padrao": {"nome": cur["ap"][0], "descricao_pt": cur["ap"][1]},
            "perseguicao": {"gatilho": gatilho, "efeito_pt": cur["per"][1], "aplica": aplica},
            "passiva": {"nome": cur["pas"][0], "descricao_pt": cur["pas"][1]},
            "passiva_chakra": {"nome": cur["cha"][0], "descricao_pt": cur["cha"][1]},
        })

    out = {
        "versao": 1,
        "fonte": "Naruto Online (banco sistema-dominus, research/fresh_ninja_list_clean.json, 255 ninjas)",
        "normalizacao_estados": {
            "小浮空": "arremesso_leve", "大浮空": "arremesso",
            "击退": "empurrao", "击倒": "derrubada",
            "obs": "gatilhos por contagem de combo/condição usam 'nenhum' e são explicados em efeito_pt",
        },
        "ninjas": ninjas,
    }
    with open(SAIDA, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)

    # ---------------- estatísticas para MECANICAS.md / relatório ----------------
    print("OK ->", SAIDA, "| ninjas:", len(ninjas))
    el = collections.Counter(n["elemento"] for n in ninjas)
    gr = collections.Counter(n["grade"] for n in ninjas)
    ch = collections.Counter(n["mystery"]["chakra"] for n in ninjas)
    cd_ = collections.Counter(n["mystery"]["cooldown"] for n in ninjas)
    tg = collections.Counter(n["perseguicao"]["gatilho"] for n in ninjas)
    ap = collections.Counter(n["perseguicao"]["aplica"] for n in ninjas)
    print("elementos:", dict(el))
    print("grades:", dict(sorted(gr.items())))
    print("chakra mystery:", dict(sorted(ch.items())))
    print("cooldown mystery:", dict(sorted(cd_.items())))
    print("gatilhos:", dict(tg))
    print("aplicados:", dict(ap))
    if avisos:
        print("avisos:")
        for a in avisos:
            print(" -", a)

if __name__ == "__main__":
    main()
