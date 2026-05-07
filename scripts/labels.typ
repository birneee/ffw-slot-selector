// A4 label sheet for Avery Zweckform J8177-25 (99.1 x 42.3 mm, 2x6 per sheet).

#let border = sys.inputs.at("border", default: "true") == "true"
#let rows = csv("../benutzer.csv").slice(1)
#let tokens = rows.map(r => r.at(0)).filter(t => t != "")

#set page(
  paper: "a4",
  margin: (top: 21.6mm, bottom: 21.6mm, left: 4.65mm, right: 4.65mm),
)

#grid(
  columns: (99.1mm, 99.1mm),
  column-gutter: 2.5mm,
  rows: (42.3mm,),
  row-gutter: 0mm,
  ..range(0, tokens.len(), step: 2).map(i => {
    let t1 = tokens.at(i)
    let t2 = if i + 1 < tokens.len() { tokens.at(i + 1) } else { none }
    rect(
      width: 100%,
      height: 100%,
      radius: 3mm,
      stroke: if border { 0.5pt } else { none },
      align(center + horizon,
        stack(
          dir: ltr,
          spacing: 7.3mm,
          image("../qrcodes/" + t1 + ".png", height: 3.5cm),
          ..if t2 != none {
            (image("../qrcodes/" + t2 + ".png", height: 3.5cm),)
          } else { () },
        )
      )
    )
  })
)
