use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let comp_ident = input.ident;

    let expanded = quote! {
        impl ::ecs::component::Component for #comp_ident {

        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let res_ident = input.ident;

    let expanded = quote! {
        impl ::ecs::resource::Resource for #res_ident {
            #[inline]
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            #[inline]
            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }

            #[inline]
            fn into_any(self: Box<Self>) -> Box<dyn ::std::any::Any> {
                self
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(ScheduleLabel)]
pub fn derive_schedule_label(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let sched_ident = input.ident;

    let expanded = quote! {
        impl ::ecs::scheduler::ScheduleLabel for #sched_ident {
            const NAME: &'static str = stringify!(#sched_ident);
        }
    };

    TokenStream::from(expanded)
}
