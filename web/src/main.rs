fn main() {
    dioxus_web::launch(backend::crossinfo_app_structure);
}

// use dioxus::prelude::*;

// struct ThirdPartyStruct(i64);

// impl ThirdPartyStruct {
//     fn modify(&mut self, offset: i64) {
//         self.0 += offset;
//     }
// }

// fn app(cx: Scope) -> Element {
//     let third_party_struct = use_state(cx, || ThirdPartyStruct(0));

//     cx.render(rsx! {
//         button {
//             onclick: move |_| third_party_struct.make_mut().modify(1),
//             "Click me!"
//         }
//         p {
//             "{third_party_struct.get().0}"
//         }
//     })
// }
