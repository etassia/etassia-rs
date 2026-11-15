# The Diagram Interchange Model (DIM) — draft 0.1

Status: draft for review. This document defines the interchange that lets
model-holding tools draw through Etassia without Etassia parsing their
models. It is the boundary between the frontends and the engine.

## 1. Purpose

Etassia reads diagram sources through its notation registry: the Mermaid
and PlantUML families claim and parse their own text. Diagrams, however,
are also derived from models — XML Schema, JSON Schema, UML, EXPRESS — and
the authoritative parsers of those models live in the LutaML stack
(lutaml-xsd, lutaml-jsonschema, ea, lutaml-express), not inside the engine.

The Diagram Interchange Model (DIM) is the contract between those two
worlds. A model tool walks its own object model, emits a DIM document, and
hands it to the engine over the same C ABI the text notations use. The
engine never parses a schema; the model tool never lays out or renders.
Each side changes on its own schedule, held together by the version stamps
in every DIM document.

DIM is deliberately a superset of the ELK JSON graph that elkjs and elkrb
speak: positions and geometry come from the ELK vocabulary, and the DIM
extensions add what rendering needs beyond layout — content, style
intent, and provenance. A graph that elkrb lays out today is already a
valid DIM document.

## 2. Document shape

A DIM document is a JSON object with three members:

- `dim` — the literal string `"0.1"`, the version of this specification
  the document was written against;
- `notation` — the identifier of the family the view expresses, for
  example `"uml-class"` or `"express-g"`, carried for provenance and for
  renderer hints; the identifier set is open;
- `graph` — an ELK JSON graph object (`id`, `children`, `edges`,
  `layoutOptions`), extended by the members defined in section 3.

The engine entry is the registry notation `dim`: a source whose detected
notation is `dim` is parsed as a DIM document. Text notations keep their
own entries; DIM is their peer, not their replacement.

## 3. Extensions over ELK JSON

### 3.1 Node extension

An ELK node gains one optional member:

- `content` — an array of blocks, drawn top to bottom inside the node
  rectangle. A block is `{"kind": "label", "text": "...", "role":
  "title"|"member"|"stereotype"|"note"}` in this draft; further kinds
  (compartments, headers) are expected and are added by version bumps,
  never by side channels.

### 3.2 Edge extension

An ELK edge gains two optional members:

- `label` — `{"text": "..."}`, rendered beside the edge midpoint;
- `style` — `"association" | "generalization" | "composition" |
  "dependency" | "reference"`, the semantic intent of the line. The
  renderer maps the intent to its marker and stroke vocabulary; the DIM
  does not carry pixel styling.

### 3.3 Layout options

Layout options are the ELK option strings on the graph's
`layoutOptions`, including `elk.algorithm` (whose value selects the
engine's layout crate, `layered` today). A DIM author may pin layout
behavior, but positions themselves are computed, not carried: a DIM
document describes what to draw, never where it lands.

## 4. Version stamping and the reproducibility contract

A rendered diagram is a function of three versions together:

1. the DIM document (its `dim` version and its content);
2. the layout engine and algorithm (the elkr crate version and the
   algorithm name);
3. the renderer (the etassia runtime version).

The engine reports this triple with its output, and the byte-identity
guarantee holds within one triple. A change to any member of the triple
is a rendering change and is released as such. Consumers that require
reproducible output pin the triple; consumers that accept improvement
update it deliberately.

## 5. Examples

### 5.1 A UML class view

```json
{
  "dim": "0.1",
  "notation": "uml-class",
  "graph": {
    "id": "root",
    "layoutOptions": { "elk.algorithm": "layered", "elk.direction": "DOWN" },
    "children": [
      {
        "id": "c-address",
        "width": 180, "height": 90,
        "content": [
          { "kind": "label", "text": "Address", "role": "title" },
          { "kind": "label", "text": "+ street: String", "role": "member" },
          { "kind": "label", "text": "+ city: String", "role": "member" }
        ]
      },
      {
        "id": "c-person",
        "width": 180, "height": 110,
        "content": [
          { "kind": "label", "text": "Person", "role": "title" },
          { "kind": "label", "text": "+ name: String", "role": "member" },
          { "kind": "label", "text": "+ address: Address", "role": "member" }
        ]
      }
    ],
    "edges": [
      {
        "id": "e-person-address",
        "sources": ["c-person"], "targets": ["c-address"],
        "style": "association",
        "label": { "text": "lives at" }
      }
    ]
  }
}
```

### 5.2 Relation to ELK JSON

An ELK JSON document that elkrb consumes is a DIM document with the
`content`, `label`, and `style` members absent and the `notation` set to
`"generic"`. The mapping table:

| ELK JSON | DIM | Meaning |
|---|---|---|
| `id`, `children`, `edges`, `layoutOptions` | identical | the layout surface |
| `x`, `y` on nodes | carried, ignored on input | layout output only |
| — | `dim` | specification version |
| — | `notation` | the expressing family |
| — | `content[]` | what to draw inside a node |
| — | `label`, `style` on edges | the semantic intent of a line |

## 6. What DIM deliberately does not carry

DIM carries no pixel styling: colors, fonts, and stroke widths belong to
the renderer and its themes, so that one document draws consistently in
every theme and the rendering remains a function of the triple in
section 4. DIM carries no layout results, because positions that arrive
with a document would silently break the reproducibility contract.
DIM carries no parser semantics: a DIM document never encodes "this node
is an xsd:complexType" — it encodes the drawn formation the model tool
derived from that fact.
