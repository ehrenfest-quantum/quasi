# DFS-Pairing im Solvayeur-Qubit-Mapping

**Status:** Konzept / Design-Skizze — noch kein Code.
**Bezug:** Solvayeur-Kernel-Roadmap Phase B (Qubit Region Allocation) + Phase F (Error Budget).
**Ausgangs-Intuition:** „Können zwei gekoppelte Qubits sich gegenseitig korrigieren — ähnlich wie Noise-Cancelling, wo einer die Störung des anderen übernimmt?"

Dieses Dokument hält die Antwort fest und übersetzt sie in eine konkrete,
umsetzbare Erweiterung des bestehenden `quasi-solvayeur/src/qubit_map.rs`.

---

## 1. Die physikalische Kernaussage

Aktives Noise-Cancelling (Audio) funktioniert, weil die Störung **messbar,
kopierbar und in eine Senke ableitbar** ist: gemessenes Signal → invertiert →
in den Gegenschall-Lautsprecher. Die Energie/Entropie der Störung fließt ab.

Bei zwei rein **kohärent gekoppelten** Qubits (ein `J·ZᵢZⱼ`-Term, eine
Gatterwechselwirkung) geht das **nicht**: eine unitäre Kopplung ist
entropieerhaltend. Sie verschiebt den Fehler zwischen den Qubits, entfernt ihn
aber nicht. „Noise-Cancelling" braucht eine Senke — und dafür gibt es genau
zwei physikalisch reale Wege:

| Weg | Prinzip | Bedingung | Reifegrad |
|-----|---------|-----------|-----------|
| **A — DFS** (Decoherence-Free Subspace) | Dem Rauschen *ausweichen*: logische Info in einen Unterraum legen, den das Rauschen invariant lässt | Rauschen muss zwischen den Qubits **korreliert** sein (gleicher Bath) | Etabliert |
| **B — Autonome/dissipative QEC** | Entropie aktiv *rauspumpen*: Partnerqubit kontinuierlich dissipieren/zurücksetzen, System relaxiert in den Code-Raum | Engineered dissipation / Reservoir-Kopplung | Forschung (Cat-Qubits, bosonische Codes) |

Weg A ist der Intuition am nächsten und **heute im Solvayeur-Mapping
umsetzbar**, ohne Hardware-Änderungen. Weg B ist hardware-nah und gehört, wenn
überhaupt, hinter die HAL-Contract-Grenze — nicht in den Compiler/Kernel.

### Beispiel DFS (kollektives Dephasing)

Wirkt ein Dephasing-Fehler **kollektiv** auf beide Qubits, dann sammeln die
Zustände `|01⟩` und `|10⟩` dieselbe Phase — die sich in der Relativphase
weghebt. Ein logisches Qubit, kodiert in `span{|01⟩, |10⟩}`, „sieht" diesen
Fehler nie. Das ist echtes gegenseitiges Schützen — aber es *cancelt* nicht, es
*dodged*.

---

## 2. Die harte Grenze (ehrlich)

Zwei (oder vier) Qubits allein können **keinen beliebigen** Ein-Qubit-Fehler
*korrigieren*. Die Quanten-Singleton-Schranke verlangt für Korrektur beliebiger
Fehler Distanz 3 → minimal `[[5,1,3]]`. Mit zwei/vier Qubits geht nur:

- **Detektion** (`[[4,2,2]]`), oder
- **Korrektur eines eingeschränkten Fehlersatzes** — und genau dieser
  eingeschränkte Satz ist der korrelierte/kollektive Fall (Weg A).

Die Intuition ist also **exakt für das Regime richtig, in dem sie überhaupt
funktionieren kann**: korreliertes Rauschen. Bei unabhängigem Rauschen auf
beiden Qubits bricht DFS zusammen — dann führt kein Weg an vollem QEC
(`[[5,1,3]]`, Surface Code) vorbei.

---

## 3. Warum das gut zu QUASI passt

DFS ist eine **Hamiltonian-Aussage**: kollektive Noise-Terme und die
Paar-Kopplung sind exakt die Ising/Ehrenfest-Sprache, die Afana ohnehin
kompiliert. Und der Solvayeur trifft bereits genau die Art Entscheidung, die
DFS-Pairing braucht: *welche physischen Qubits werden wofür belegt* (Phase B).

Der heutige `qubit_map.rs`-Ansatz **meidet** schlechte/korrelierte Regionen.
DFS-Pairing dreht das für einen Spezialfall um: **gezielt Qubit-Paare mit
korreliertem Rauschen zusammenlegen und im DFS kodieren** — ein Nachteil
(Crosstalk-Korrelation) wird zum Schutzmechanismus.

---

## 4. Konkrete Phase-B-Erweiterung

### 4.1 Was heute existiert

`BackendCalibration` (in `quasi-solvayeur/src/calibration.rs`) liefert
pro-Qubit `t1/t2/gate_fidelity/readout_fidelity` und pro-Edge
`gate_fidelity`. `qubit_map.rs` wählt daraus eine Region über `qubit_score`
(Qualität) und `region_score` (Qualität + Konnektivität).

**Was fehlt:** ein Maß für **Rausch-Korrelation** zwischen Qubit-Paaren. Ohne
das kann der Kernel DFS-taugliche Paare nicht erkennen.

### 4.2 Neue Kalibrierungsgröße: Korrelation

`BackendCalibration` um pro-Paar-Korrelation erweitern (aus HAL Contract, sobald
verfügbar; bis dahin mock-basiert):

```rust
/// Korrelation des Dephasing-Rauschens zwischen zwei Qubits, in [0, 1].
/// 1.0 = perfekt korreliert (gleicher Bath) → DFS-tauglich.
/// 0.0 = unabhängig → DFS wirkungslos.
pub struct NoiseCorrelation {
    pub qubit_a: u32,
    pub qubit_b: u32,
    pub dephasing_corr: f64,
}
```

Quelle in echter Hardware: simultane Ramsey-/Echo-Messungen benachbarter
Qubits, gemeinsame TLS-/Frequenznachbarn, Crosstalk-Matrix. Das kommt über
`GET /hal/backends/{name}/calibration` — **nicht** aus dem Compiler.

### 4.3 Neues Modul: `dfs_pairing.rs`

```
quasi-solvayeur/src/dfs_pairing.rs
```

Aufgaben:

1. **Paar-Scoring:** `dfs_score(a, b) = dephasing_corr(a,b) · min(quality(a), quality(b))`
   — ein Paar ist nur wertvoll, wenn es *sowohl* stark korreliert *als auch*
   einzeln brauchbar ist.
2. **Paarauswahl:** aus der Korrelationsmatrix ein Maximum-Weight-Matching
   über die kandidierenden physischen Qubits (klassisch, in Rust — passt zur
   Afana/Huoma-Philosophie, kein Vendor-SDK). Ergebnis: eine Menge disjunkter
   DFS-Paare.
3. **Logisches Mapping:** jedes DFS-Paar wird *ein* logisches Qubit im
   `QubitMap`. Der bestehende `QubitMap.mapping` wird um eine Ebene ergänzt
   (virtuelles logisches Qubit → Paar physischer Qubits).

### 4.4 Hamiltonian-Term (ATW-Integration)

Als Erweiterung des Phase-B-Hamiltonians `H_B`:

```
H_dfs = H_B  −  Σ_{(p,q) ∈ Paare}  κ · corr(p,q) · Z_p Z_q
```

Das negative Vorzeichen belohnt Messausgänge, die stark korrelierte Qubits
*gemeinsam* für ein logisches Qubit belegen. Wie bei allen Solvayeur-Termen
lernt der Bias-Update aus dem beobachteten Reward (hier: gemessene logische
Fehlerrate des DFS-kodierten Paars) — der Kernel kalibriert `κ` implizit auf
das reale Rauschen des Chips.

---

## 5. Abgrenzung — was das NICHT ist

- **Kein Ersatz für Surface Code.** DFS-Pairing schützt nur gegen korreliertes
  Dephasing. Für allgemeine Fehler bleibt vollwertiges QEC nötig. DFS ist eine
  *billige erste Schicht* (2 physische → 1 logisches Qubit), kein Distanz-d-Code.
- **Kein Hardware-Eingriff.** Weg B (dissipative QEC) wird hier bewusst
  ausgeklammert — er gehört hinter die HAL-Contract-Grenze, nicht in Kernel
  oder Compiler.
- **Kein QPU-Echtzeit-Decoder.** Dies ist eine *Encoding-/Placement*-
  Entscheidung im langsamen Outer-Loop, kein µs-Inner-Loop-Decoding.

---

## 6. Umsetzungsschritte (wenn wir es bauen)

1. `NoiseCorrelation` + `correlations: Vec<NoiseCorrelation>` in
   `BackendCalibration`; `mock()` erzeugt eine plausible Korrelationsmatrix
   (z. B. hohe Korrelation zwischen topologischen Nachbarn).
2. `dfs_pairing.rs`: `dfs_score`, Maximum-Weight-Matching, Paar→logisch-Mapping,
   Unit-Tests (analog zum Teststil in `qubit_map.rs`).
3. HAL-Contract-Erweiterung dokumentieren: `dephasing_corr` im
   Calibration-Endpoint (Prerequisite, wie bei Phase B die pro-Qubit-Kalibrierung).
4. Optional: Ehrenfest-Beispiel `dfs_pair_2q` analog zu `toric_code_8q`, das
   das kollektive-Dephasing-Encoding als Hamiltonian ausdrückt.

---

*Kurzfassung: Die Intuition stimmt — sobald man das fehlende Stück ergänzt.
Rauschen braucht eine Senke. Ohne Senke schiebt reine Kopplung den Fehler nur
herum; mit korreliertem Rauschen weicht man ihm via DFS aus; mit Dissipation
pumpt man ihn raus (autonome QEC, hinter HAL). Für QUASI ist DFS-Pairing der
umsetzbare Weg: eine Phase-B-Erweiterung, die korreliertes Rauschen von einem
Nachteil in einen Schutzmechanismus verwandelt.*
