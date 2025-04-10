use crate::widgets::toastify::ToastifyOptions;
use yew::prelude::*;

use super::{BitcoinQrCode, QrCornersDotType, QrCornersSquareType, QrDotsType, QrType};

#[function_component(BitcoinQrTest)]
pub fn bitcoin_qr_test() -> Html {
    let sample_invoice = "lnbc1500n1pj9xm78pp5yztkwjcz5ftl5laxkav23zmzekaw37zk6kmv80k4xavw7518gsdqqcqzzsxqyz5vqsp5usyc4lk9chsfp53kvcn7k3c7yfwzqwaeyafpjh2gk3m38kgtu8q9qyyssqjpsl9x304vx4qx74fcgwuh2m6aqk7tacu5meg8de3gg0d4mukh95js3jl0tf3zqppfu6wfc8k30amxkf3kx3j6sm3923l5nfj8eygsp2t3qc2";
    let sample_bitcoin_address = "bc1qylh3u67j673h6y6alv70m0pl2yz53tzhvxgg7u";

    let copy_invoice = {
        let invoice = sample_invoice.to_string();
        Callback::from(move |_| {
            crate::browser_api::clipboard_copy(&invoice);
            ToastifyOptions::new_success("Lightning invoice copied to clipboard").show();
        })
    };

    let copy_address = {
        let address = sample_bitcoin_address.to_string();
        Callback::from(move |_| {
            crate::browser_api::clipboard_copy(&address);
            ToastifyOptions::new_success("Bitcoin address copied to clipboard").show();
        })
    };

    html! {
        <div class="flex flex-col gap-10 items-center p-4">
            <h1 class="text-2xl font-bold">{"Bitcoin QR Code Demo"}</h1>

            <div class="flex flex-wrap justify-center gap-8">
                <div class="bg-zinc-100 p-6 rounded-2xl flex flex-col items-center gap-4 max-w-xs">
                    <h3 class="text-xl font-bold">{"Lightning Invoice"}</h3>
                    <p class="text-sm text-gray-600 text-center">{"Scan this QR code with a Lightning wallet to test"}</p>

                    <BitcoinQrCode
                        id={"lightning-qr".to_string()}
                        width={"200".to_string()}
                        height={"200".to_string()}
                        lightning={sample_invoice.to_string()}
                        type_={Some(QrType::Svg)}
                        corners_square_type={Some(QrCornersSquareType::ExtraRounded)}
                        corners_square_color={Some("#B40A2D".to_string())}
                        corners_dot_color={Some("#ECC81D".to_string())}
                        dots_type={Some(QrDotsType::ClassyRounded)}
                        dots_color={Some("#377E3F".to_string())}
                    />

                    <button
                        onclick={copy_invoice}
                        class="mt-2 bg-blue-500 text-white font-bold py-2 px-4 rounded hover:bg-blue-600 w-full">
                        {"Copy Invoice"}
                    </button>
                </div>

                <div class="bg-zinc-100 p-6 rounded-2xl flex flex-col items-center gap-4 max-w-xs">
                    <h3 class="text-xl font-bold">{"Bitcoin Address"}</h3>
                    <p class="text-sm text-gray-600 text-center">{"Scan this QR code with a Bitcoin wallet to test"}</p>

                    <BitcoinQrCode
                        id={"bitcoin-qr".to_string()}
                        width={"200".to_string()}
                        height={"200".to_string()}
                        bitcoin={sample_bitcoin_address.to_string()}
                        type_={Some(QrType::Svg)}
                        dots_type={Some(QrDotsType::Rounded)}
                        dots_color={Some("#FF9900".to_string())}
                        corners_square_type={Some(QrCornersSquareType::Dot)}
                        corners_square_color={Some("#FF9900".to_string())}
                        corners_dot_type={Some(QrCornersDotType::Dot)}
                        corners_dot_color={Some("#FF9900".to_string())}
                    />

                    <button
                        onclick={copy_address}
                        class="mt-2 bg-orange-500 text-white font-bold py-2 px-4 rounded hover:bg-orange-600 w-full">
                        {"Copy Address"}
                    </button>
                </div>
            </div>

            <div class="mt-6 text-sm text-gray-600 max-w-2xl text-center">
                <p>{"The Bitcoin QR component allows for flexible customization of QR code appearance and behavior. All styling parameters can be customized through props."}</p>
                <p class="mt-2">{"This component is a wrapper around the bitcoin-qr web component, which provides QR code generation for Bitcoin on-chain, Lightning, and unified BIP-21 payments."}</p>
            </div>
        </div>
    }
}
