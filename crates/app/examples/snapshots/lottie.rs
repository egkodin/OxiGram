//! Lottie fixtures: a round face that rocks and pulses for one second, in
//! a few colors, written as plain Lottie JSON (the player takes TGS
//! gzipped or not).

use std::path::PathBuf;

/// Fill colors, as Lottie's 0 to 1 RGB.
const COLORS: [[f32; 3]; 8] = [
    [0.98, 0.78, 0.24],
    [0.36, 0.68, 0.98],
    [0.95, 0.42, 0.45],
    [0.45, 0.82, 0.52],
    [0.72, 0.52, 0.95],
    [0.98, 0.58, 0.28],
    [0.32, 0.82, 0.86],
    [0.92, 0.45, 0.78],
];

/// The animation's JSON for `variant`.
fn json(variant: usize) -> String {
    let [r, g, b] = COLORS[variant % COLORS.len()];
    let ease = r#""i":{"x":[0.5],"y":[1]},"o":{"x":[0.5],"y":[0]}"#;
    let ease3 = r#""i":{"x":[0.5,0.5,0.5],"y":[1,1,1]},"o":{"x":[0.5,0.5,0.5],"y":[0,0,0]}"#;
    let fill = |r: f32, g: f32, b: f32| {
        format!(r#"{{"ty":"fl","c":{{"a":0,"k":[{r},{g},{b},1]}},"o":{{"a":0,"k":100}},"r":1}}"#)
    };
    let circle = |x: i32, y: i32, side: i32| {
        format!(
            r#"{{"ty":"el","d":1,"p":{{"a":0,"k":[{x},{y}]}},"s":{{"a":0,"k":[{side},{side}]}}}}"#
        )
    };
    let transform = r#"{"ty":"tr","p":{"a":0,"k":[0,0]},"a":{"a":0,"k":[0,0]},"s":{"a":0,"k":[100,100]},"r":{"a":0,"k":0},"o":{"a":0,"k":100}}"#;
    let group = |items: Vec<String>| format!(r#"{{"ty":"gr","it":[{}]}}"#, items.join(","));
    let eyes = group(vec![
        circle(-70, -50, 60),
        circle(70, -50, 60),
        fill(0.24, 0.16, 0.08),
        transform.to_owned(),
    ]);
    let face = group(vec![circle(0, 0, 440), fill(r, g, b), transform.to_owned()]);
    format!(
        r#"{{"v":"5.7.4","fr":60,"ip":0,"op":60,"w":512,"h":512,"nm":"face","ddd":0,"assets":[],"layers":[
{{"ddd":0,"ind":1,"ty":4,"nm":"face","sr":1,"ip":0,"op":60,"st":0,"bm":0,"ao":0,
"ks":{{"o":{{"a":0,"k":100}},
"r":{{"a":1,"k":[{{"t":0,"s":[-12],{ease}}},{{"t":30,"s":[12],{ease}}},{{"t":60,"s":[-12]}}]}},
"p":{{"a":0,"k":[256,256,0]}},"a":{{"a":0,"k":[0,0,0]}},
"s":{{"a":1,"k":[{{"t":0,"s":[88,88,100],{ease3}}},{{"t":30,"s":[100,100,100],{ease3}}},{{"t":60,"s":[88,88,100]}}]}}}},
"shapes":[{eyes},{face}]}}]}}"#
    )
}

/// The fixture's path for `variant`, written once per run.
pub(crate) fn tgs(variant: usize) -> Option<PathBuf> {
    let folder = std::env::temp_dir().join("oxigram-snapshots");
    let path = folder.join(format!("face-{variant}.tgs"));
    std::fs::create_dir_all(&folder)
        .and_then(|()| std::fs::write(&path, json(variant)))
        .inspect_err(|error| eprintln!("writing the Lottie fixture: {error:#}"))
        .ok()
        .map(|()| path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_variant_opens_and_moves() {
        for variant in 0..super::COLORS.len() {
            let mut animation = media::lottie::Animation::from_bytes(
                super::json(variant).as_bytes(),
                Default::default(),
            )
            .expect("the fixture is Lottie");
            assert_eq!(animation.frame_count(), 60);
            let first = animation.render(0, 64, 64).expect("frame 0 renders");
            let middle = animation.render(30, 64, 64).expect("frame 30 renders");
            assert_ne!(first, middle, "the face moves");
        }
    }
}
