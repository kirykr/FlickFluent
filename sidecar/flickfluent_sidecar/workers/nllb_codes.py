"""NLLB-200 writes a language as a FLORES-200 token such as `khm_Khmr`. This maps our codes
(ISO 639-1, or ISO 639-3 where there is no two-letter code) to those tokens. A test checks every
token against the model's vocabulary."""

from __future__ import annotations

NLLB_TOKENS = {
    "af": "afr_Latn", "am": "amh_Ethi", "ar": "arb_Arab", "as": "asm_Beng", "ast": "ast_Latn",
    "ay": "ayr_Latn", "az": "azj_Latn", "ba": "bak_Cyrl", "be": "bel_Cyrl", "bg": "bul_Cyrl",
    "bn": "ben_Beng", "bo": "bod_Tibt", "bs": "bos_Latn", "ca": "cat_Latn", "ceb": "ceb_Latn",
    "cs": "ces_Latn", "cy": "cym_Latn", "da": "dan_Latn", "de": "deu_Latn", "el": "ell_Grek",
    "en": "eng_Latn", "es": "spa_Latn", "et": "est_Latn", "eu": "eus_Latn", "fa": "pes_Arab",
    "fi": "fin_Latn", "fo": "fao_Latn", "fr": "fra_Latn", "ga": "gle_Latn", "gd": "gla_Latn",
    "gl": "glg_Latn", "gn": "grn_Latn", "gu": "guj_Gujr", "ha": "hau_Latn", "he": "heb_Hebr",
    "hi": "hin_Deva", "hr": "hrv_Latn", "ht": "hat_Latn", "hu": "hun_Latn", "hy": "hye_Armn",
    "id": "ind_Latn", "ig": "ibo_Latn", "ilo": "ilo_Latn", "is": "isl_Latn", "it": "ita_Latn",
    "ja": "jpn_Jpan", "jv": "jav_Latn", "ka": "kat_Geor", "kk": "kaz_Cyrl", "km": "khm_Khmr",
    "kn": "kan_Knda", "ko": "kor_Hang", "ku": "kmr_Latn", "ky": "kir_Cyrl", "lb": "ltz_Latn",
    "lg": "lug_Latn", "ln": "lin_Latn", "lo": "lao_Laoo", "lt": "lit_Latn", "lv": "lvs_Latn",
    "mg": "plt_Latn", "mi": "mri_Latn", "mk": "mkd_Cyrl", "ml": "mal_Mlym", "mn": "khk_Cyrl",
    "mr": "mar_Deva", "ms": "zsm_Latn", "mt": "mlt_Latn", "my": "mya_Mymr", "ne": "npi_Deva",
    "nl": "nld_Latn", "no": "nob_Latn", "oc": "oci_Latn", "om": "gaz_Latn", "or": "ory_Orya",
    "pa": "pan_Guru", "pl": "pol_Latn", "ps": "pbt_Arab", "pt": "por_Latn", "qu": "quy_Latn",
    "ro": "ron_Latn", "ru": "rus_Cyrl", "sa": "san_Deva", "sd": "snd_Arab", "si": "sin_Sinh",
    "sk": "slk_Latn", "sl": "slv_Latn", "sn": "sna_Latn", "so": "som_Latn", "sq": "als_Latn",
    "sr": "srp_Cyrl", "ss": "ssw_Latn", "st": "sot_Latn", "su": "sun_Latn", "sv": "swe_Latn",
    "sw": "swh_Latn", "ta": "tam_Taml", "te": "tel_Telu", "tg": "tgk_Cyrl", "th": "tha_Thai",
    "ti": "tir_Ethi", "tk": "tuk_Latn", "tl": "tgl_Latn", "tn": "tsn_Latn", "tr": "tur_Latn",
    "tt": "tat_Cyrl", "uk": "ukr_Cyrl", "ur": "urd_Arab", "uz": "uzn_Latn", "vi": "vie_Latn",
    "wo": "wol_Latn", "xh": "xho_Latn", "yo": "yor_Latn", "zh": "zho_Hans", "zu": "zul_Latn",
}
