use super::*;
use crate::ast::Parameter;

#[derive(Debug, Clone, Copy)]
pub(super) enum Builtin {
    Exit,
    Clone,
    Strlen,
    StrRepeat,
    StrRev,
    StrLower,
    StrUpper,
    Bin2Hex,
    StrReplace(bool),
    Substr,
    SubstrReplace,
    StringSearch(string_search::SearchKind),
    StrCompare(bool, bool),
    SubstrCompare,
    SubstrCount,
    StrSpan(bool),
    Explode,
    Implode(&'static str),
    StrSplit,
    ChunkSplit,
    StrTranslate,
    SlashEscape(bool, bool),
    StripTags,
    Hex2Bin,
    FirstCase(bool),
    Ucwords,
    Nl2Br,
    CountChars,
    StrPad,
    StrPbrk,
    Trim(u8, &'static str),
    Chr,
    Ord,
    Count,
    VarDump,
    PrintR,
    VarExport,
    Printf,
    Sprintf,
    Vprintf,
    Vsprintf,
    GetType,
    GetDebugType,
    ResourceInfo(bool),
    Fopen,
    FileExists,
    Unlink,
    Stream(streams::Operation),
    GetClass,
    GetParentClass,
    GetCalledClass,
    ClassExists,
    EnumExists,
    ClassAlias,
    IsA,
    IsSubclassOf,
    InterfaceExists,
    TraitExists,
    MethodExists,
    PropertyExists,
    GetObjectVars,
    ObjectId,
    ObjectHash,
    GetClassVars,
    GetClassMethods,
    Is(&'static str),
    Range,
    Math(math::Math),
    Round,
    NumberFormat,
    BaseConvert,
    BaseDecode(u8),
    BaseEncode(u8),
    ArraySlice,
    ArraySplice,
    ArrayColumn,
    ArraySet(array_sets::Operation),
    ArrayUserSet(array_user_sets::Operation),
    ArrayStack(array_stack::Action),
    ArrayReverse,
    ArrayChunk,
    ArrayFill,
    ArrayFillKeys,
    ArrayPad,
    ArrayKeys,
    ArrayValues,
    ArrayKeyExists,
    ArraySearch(bool),
    ArrayNumeric(bool),
    ArrayEdge(bool, bool),
    ArrayIsList,
    ArrayFlip,
    ArrayCountValues,
    ArrayCombine,
    ArrayMerge(bool),
    ArrayReplace(bool),
    ArrayCursor(crate::array::Cursor),
    ArrayWalk(bool),
    MinMax(bool),
    ArraySort(sorting::Sort),
    ArrayUserSort(sorting::UserSort),
    ArrayMultisort,
    Extract,
    Compact,
    GetDefinedVars,
    ArrayMap,
    ArrayFilter,
    ArrayReduce,
    CallUserFunc,
    CallUserFuncArray,
    FunctionExists,
    IsCallable,
    DebugBacktrace,
    DebugPrintBacktrace,
    Define,
    Defined,
    Constant,
    GetIncludedFiles,
    FuncNumArgs,
    FuncGetArg,
    FuncGetArgs,
    ErrorReporting,
    AutoloadRegister,
    AutoloadUnregister,
    AutoloadFunctions,
    AutoloadCall,
    Autoload,
    AutoloadExtensions,
}
impl Builtin {
    pub(super) fn lookup(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"exit" | b"die" => Self::Exit,
            b"clone" => Self::Clone,
            b"strlen" => Self::Strlen,
            b"str_repeat" => Self::StrRepeat,
            b"strrev" => Self::StrRev,
            b"strtolower" => Self::StrLower,
            b"strtoupper" => Self::StrUpper,
            b"bin2hex" => Self::Bin2Hex,
            b"str_replace" => Self::StrReplace(false),
            b"str_ireplace" => Self::StrReplace(true),
            b"substr" => Self::Substr,
            b"substr_replace" => Self::SubstrReplace,
            b"strpos" => Self::StringSearch(string_search::SearchKind::Position),
            b"stripos" => Self::StringSearch(string_search::SearchKind::PositionInsensitive),
            b"strrpos" => Self::StringSearch(string_search::SearchKind::LastPosition),
            b"strripos" => Self::StringSearch(string_search::SearchKind::LastPositionInsensitive),
            b"strstr" => Self::StringSearch(string_search::SearchKind::Suffix),
            b"strchr" => Self::StringSearch(string_search::SearchKind::SuffixAlias),
            b"stristr" => Self::StringSearch(string_search::SearchKind::SuffixInsensitive),
            b"strrchr" => Self::StringSearch(string_search::SearchKind::LastCharacter),
            b"str_contains" => Self::StringSearch(string_search::SearchKind::Contains),
            b"str_starts_with" => Self::StringSearch(string_search::SearchKind::StartsWith),
            b"str_ends_with" => Self::StringSearch(string_search::SearchKind::EndsWith),
            b"strcmp" => Self::StrCompare(false, false),
            b"strcasecmp" => Self::StrCompare(true, false),
            b"strncmp" => Self::StrCompare(false, true),
            b"strncasecmp" => Self::StrCompare(true, true),
            b"substr_compare" => Self::SubstrCompare,
            b"substr_count" => Self::SubstrCount,
            b"strspn" => Self::StrSpan(true),
            b"strcspn" => Self::StrSpan(false),
            b"explode" => Self::Explode,
            b"implode" => Self::Implode("implode"),
            b"join" => Self::Implode("join"),
            b"str_split" => Self::StrSplit,
            b"chunk_split" => Self::ChunkSplit,
            b"strtr" => Self::StrTranslate,
            b"addslashes" => Self::SlashEscape(true, false),
            b"stripslashes" => Self::SlashEscape(false, false),
            b"addcslashes" => Self::SlashEscape(true, true),
            b"stripcslashes" => Self::SlashEscape(false, true),
            b"strip_tags" => Self::StripTags,
            b"hex2bin" => Self::Hex2Bin,
            b"ucfirst" => Self::FirstCase(true),
            b"lcfirst" => Self::FirstCase(false),
            b"ucwords" => Self::Ucwords,
            b"nl2br" => Self::Nl2Br,
            b"count_chars" => Self::CountChars,
            b"str_pad" => Self::StrPad,
            b"strpbrk" => Self::StrPbrk,
            b"trim" => Self::Trim(3, "trim"),
            b"ltrim" => Self::Trim(1, "ltrim"),
            b"rtrim" => Self::Trim(2, "rtrim"),
            b"chop" => Self::Trim(2, "chop"),
            b"chr" => Self::Chr,
            b"ord" => Self::Ord,
            b"count" | b"sizeof" => Self::Count,
            b"var_dump" => Self::VarDump,
            b"print_r" => Self::PrintR,
            b"var_export" => Self::VarExport,
            b"printf" => Self::Printf,
            b"sprintf" => Self::Sprintf,
            b"vprintf" => Self::Vprintf,
            b"vsprintf" => Self::Vsprintf,
            b"gettype" => Self::GetType,
            b"get_debug_type" => Self::GetDebugType,
            b"get_class" => Self::GetClass,
            b"get_parent_class" => Self::GetParentClass,
            b"get_called_class" => Self::GetCalledClass,
            b"class_exists" => Self::ClassExists,
            b"enum_exists" => Self::EnumExists,
            b"class_alias" => Self::ClassAlias,
            b"is_a" => Self::IsA,
            b"is_subclass_of" => Self::IsSubclassOf,
            b"interface_exists" => Self::InterfaceExists,
            b"trait_exists" => Self::TraitExists,
            b"method_exists" => Self::MethodExists,
            b"property_exists" => Self::PropertyExists,
            b"get_object_vars" => Self::GetObjectVars,
            b"spl_object_id" => Self::ObjectId,
            b"spl_object_hash" => Self::ObjectHash,
            b"get_class_vars" => Self::GetClassVars,
            b"get_class_methods" => Self::GetClassMethods,
            b"is_int" | b"is_integer" | b"is_long" => Self::Is("int"),
            b"is_float" | b"is_double" => Self::Is("float"),
            b"is_bool" => Self::Is("bool"),
            b"is_null" => Self::Is("null"),
            b"is_string" => Self::Is("string"),
            b"is_object" => Self::Is("object"),
            b"is_array" => Self::Is("array"),
            b"is_resource" => Self::Is("resource"),
            b"get_resource_id" => Self::ResourceInfo(true),
            b"get_resource_type" => Self::ResourceInfo(false),
            b"fopen" => Self::Fopen,
            b"file_exists" => Self::FileExists,
            b"unlink" => Self::Unlink,
            b"fclose" => Self::Stream(streams::Operation::Close),
            b"fread" => Self::Stream(streams::Operation::Read),
            b"fwrite" | b"fputs" => Self::Stream(streams::Operation::Write),
            b"feof" => Self::Stream(streams::Operation::Eof),
            b"ftell" => Self::Stream(streams::Operation::Tell),
            b"fseek" => Self::Stream(streams::Operation::Seek),
            b"rewind" => Self::Stream(streams::Operation::Rewind),
            b"fflush" => Self::Stream(streams::Operation::Flush),
            b"range" => Self::Range,
            b"abs" => Self::Math(math::Math::Abs),
            b"ceil" => Self::Math(math::Math::Ceil),
            b"floor" => Self::Math(math::Math::Floor),
            b"pow" => Self::Math(math::Math::Pow),
            b"fpow" => Self::Math(math::Math::Fpow),
            b"fdiv" => Self::Math(math::Math::Fdiv),
            b"fmod" => Self::Math(math::Math::Fmod),
            b"intdiv" => Self::Math(math::Math::Intdiv),
            b"sqrt" => Self::Math(math::Math::Sqrt),
            b"is_finite" => Self::Math(math::Math::IsFinite),
            b"is_infinite" => Self::Math(math::Math::IsInfinite),
            b"is_nan" => Self::Math(math::Math::IsNan),
            b"pi" => Self::Math(math::Math::Pi),
            b"sin" => Self::Math(math::Math::Sin),
            b"cos" => Self::Math(math::Math::Cos),
            b"tan" => Self::Math(math::Math::Tan),
            b"asin" => Self::Math(math::Math::Asin),
            b"acos" => Self::Math(math::Math::Acos),
            b"atan" => Self::Math(math::Math::Atan),
            b"atan2" => Self::Math(math::Math::Atan2),
            b"sinh" => Self::Math(math::Math::Sinh),
            b"cosh" => Self::Math(math::Math::Cosh),
            b"tanh" => Self::Math(math::Math::Tanh),
            b"asinh" => Self::Math(math::Math::Asinh),
            b"acosh" => Self::Math(math::Math::Acosh),
            b"atanh" => Self::Math(math::Math::Atanh),
            b"exp" => Self::Math(math::Math::Exp),
            b"expm1" => Self::Math(math::Math::Expm1),
            b"log" => Self::Math(math::Math::Log),
            b"log10" => Self::Math(math::Math::Log10),
            b"log1p" => Self::Math(math::Math::Log1p),
            b"hypot" => Self::Math(math::Math::Hypot),
            b"deg2rad" => Self::Math(math::Math::Deg2rad),
            b"rad2deg" => Self::Math(math::Math::Rad2deg),
            b"round" => Self::Round,
            b"number_format" => Self::NumberFormat,
            b"base_convert" => Self::BaseConvert,
            b"bindec" => Self::BaseDecode(2),
            b"octdec" => Self::BaseDecode(8),
            b"hexdec" => Self::BaseDecode(16),
            b"decbin" => Self::BaseEncode(2),
            b"decoct" => Self::BaseEncode(8),
            b"dechex" => Self::BaseEncode(16),
            b"array_slice" => Self::ArraySlice,
            b"array_splice" => Self::ArraySplice,
            b"array_column" => Self::ArrayColumn,
            b"array_diff" => Self::ArraySet(array_sets::Operation::Difference),
            b"array_intersect" => Self::ArraySet(array_sets::Operation::Intersection),
            b"array_diff_key" => Self::ArraySet(array_sets::Operation::DifferenceKey),
            b"array_diff_assoc" => Self::ArraySet(array_sets::Operation::DifferenceAssoc),
            b"array_intersect_key" => Self::ArraySet(array_sets::Operation::IntersectionKey),
            b"array_intersect_assoc" => Self::ArraySet(array_sets::Operation::IntersectionAssoc),
            b"array_diff_ukey" => Self::ArrayUserSet(array_user_sets::Operation::DiffKeys),
            b"array_intersect_ukey" => {
                Self::ArrayUserSet(array_user_sets::Operation::IntersectKeys)
            }
            b"array_diff_uassoc" => Self::ArrayUserSet(array_user_sets::Operation::DiffAssoc),
            b"array_intersect_uassoc" => {
                Self::ArrayUserSet(array_user_sets::Operation::IntersectAssoc)
            }
            b"array_udiff" => Self::ArrayUserSet(array_user_sets::Operation::DiffValues),
            b"array_uintersect" => Self::ArrayUserSet(array_user_sets::Operation::IntersectValues),
            b"array_udiff_assoc" => Self::ArrayUserSet(array_user_sets::Operation::DiffAssocValues),
            b"array_uintersect_assoc" => {
                Self::ArrayUserSet(array_user_sets::Operation::IntersectAssocValues)
            }
            b"array_udiff_uassoc" => Self::ArrayUserSet(array_user_sets::Operation::DiffBoth),
            b"array_uintersect_uassoc" => {
                Self::ArrayUserSet(array_user_sets::Operation::IntersectBoth)
            }
            b"array_push" => Self::ArrayStack(array_stack::Action::Push),
            b"array_pop" => Self::ArrayStack(array_stack::Action::Pop),
            b"array_shift" => Self::ArrayStack(array_stack::Action::Shift),
            b"array_unshift" => Self::ArrayStack(array_stack::Action::Unshift),
            b"array_reverse" => Self::ArrayReverse,
            b"array_chunk" => Self::ArrayChunk,
            b"array_fill" => Self::ArrayFill,
            b"array_fill_keys" => Self::ArrayFillKeys,
            b"array_pad" => Self::ArrayPad,
            b"array_keys" => Self::ArrayKeys,
            b"array_values" => Self::ArrayValues,
            b"array_map" => Self::ArrayMap,
            b"array_filter" => Self::ArrayFilter,
            b"array_reduce" => Self::ArrayReduce,
            b"call_user_func" => Self::CallUserFunc,
            b"call_user_func_array" => Self::CallUserFuncArray,
            b"array_key_exists" | b"key_exists" => Self::ArrayKeyExists,
            b"in_array" => Self::ArraySearch(false),
            b"array_search" => Self::ArraySearch(true),
            b"array_sum" => Self::ArrayNumeric(false),
            b"array_product" => Self::ArrayNumeric(true),
            b"array_key_first" => Self::ArrayEdge(false, true),
            b"array_key_last" => Self::ArrayEdge(true, true),
            b"array_first" => Self::ArrayEdge(false, false),
            b"array_last" => Self::ArrayEdge(true, false),
            b"array_is_list" => Self::ArrayIsList,
            b"array_flip" => Self::ArrayFlip,
            b"array_count_values" => Self::ArrayCountValues,
            b"array_combine" => Self::ArrayCombine,
            b"array_merge" => Self::ArrayMerge(false),
            b"array_merge_recursive" => Self::ArrayMerge(true),
            b"array_replace" => Self::ArrayReplace(false),
            b"array_replace_recursive" => Self::ArrayReplace(true),
            b"array_walk" => Self::ArrayWalk(false),
            b"array_walk_recursive" => Self::ArrayWalk(true),
            b"compact" => Self::Compact,
            b"get_defined_vars" => Self::GetDefinedVars,
            b"extract" => Self::Extract,
            b"array_multisort" => Self::ArrayMultisort,
            b"usort" => Self::ArrayUserSort(sorting::UserSort::Values),
            b"uasort" => Self::ArrayUserSort(sorting::UserSort::Associative),
            b"uksort" => Self::ArrayUserSort(sorting::UserSort::Keys),
            b"sort" => Self::ArraySort(sorting::Sort::Values),
            b"rsort" => Self::ArraySort(sorting::Sort::ReverseValues),
            b"asort" => Self::ArraySort(sorting::Sort::Associative),
            b"arsort" => Self::ArraySort(sorting::Sort::ReverseAssociative),
            b"ksort" => Self::ArraySort(sorting::Sort::Keys),
            b"krsort" => Self::ArraySort(sorting::Sort::ReverseKeys),
            b"natsort" => Self::ArraySort(sorting::Sort::Natural),
            b"natcasesort" => Self::ArraySort(sorting::Sort::NaturalCase),
            b"min" => Self::MinMax(false),
            b"max" => Self::MinMax(true),
            b"current" | b"pos" => Self::ArrayCursor(crate::array::Cursor::Current),
            b"key" => Self::ArrayCursor(crate::array::Cursor::Key),
            b"reset" => Self::ArrayCursor(crate::array::Cursor::Reset),
            b"end" => Self::ArrayCursor(crate::array::Cursor::End),
            b"next" => Self::ArrayCursor(crate::array::Cursor::Next),
            b"prev" => Self::ArrayCursor(crate::array::Cursor::Prev),
            b"function_exists" => Self::FunctionExists,
            b"is_callable" => Self::IsCallable,
            b"debug_backtrace" => Self::DebugBacktrace,
            b"debug_print_backtrace" => Self::DebugPrintBacktrace,
            b"define" => Self::Define,
            b"defined" => Self::Defined,
            b"constant" => Self::Constant,
            b"get_included_files" | b"get_required_files" => Self::GetIncludedFiles,
            b"func_num_args" => Self::FuncNumArgs,
            b"func_get_arg" => Self::FuncGetArg,
            b"func_get_args" => Self::FuncGetArgs,
            b"error_reporting" => Self::ErrorReporting,
            b"spl_autoload_register" => Self::AutoloadRegister,
            b"spl_autoload_unregister" => Self::AutoloadUnregister,
            b"spl_autoload_functions" => Self::AutoloadFunctions,
            b"spl_autoload_call" => Self::AutoloadCall,
            b"spl_autoload" => Self::Autoload,
            b"spl_autoload_extensions" => Self::AutoloadExtensions,
            _ => return None,
        })
    }
    pub(super) fn parameters(self) -> Vec<Parameter> {
        let definitions: &[(&str, bool, bool)] = match self {
            Self::ResourceInfo(_) => &[("resource", false, false)],
            Self::FileExists => &[("filename", false, false)],
            Self::Unlink => &[("filename", false, false), ("context", true, false)],
            Self::Fopen => &[
                ("filename", false, false),
                ("mode", false, false),
                ("use_include_path", true, false),
                ("context", true, false),
            ],
            Self::Stream(streams::Operation::Read) => {
                &[("stream", false, false), ("length", false, false)]
            }
            Self::Stream(streams::Operation::Write) => &[
                ("stream", false, false),
                ("data", false, false),
                ("length", true, false),
            ],
            Self::Stream(streams::Operation::Seek) => &[
                ("stream", false, false),
                ("offset", false, false),
                ("whence", true, false),
            ],
            Self::Stream(_) => &[("stream", false, false)],
            Self::GetClass => &[("object", true, false)],
            Self::Clone => &[("object", false, false), ("withProperties", true, false)],
            Self::GetParentClass => &[("object_or_class", true, false)],
            Self::GetCalledClass => &[],
            Self::GetClassVars => &[("class", false, false)],
            Self::GetClassMethods => &[("object_or_class", false, false)],
            Self::GetObjectVars | Self::ObjectId | Self::ObjectHash => &[("object", false, false)],
            Self::EnumExists => &[("enum", false, false), ("autoload", true, false)],
            Self::ClassExists => &[("class", false, false), ("autoload", true, false)],
            Self::ClassAlias => &[
                ("class", false, false),
                ("alias", false, false),
                ("autoload", true, false),
            ],
            Self::IsA | Self::IsSubclassOf => &[
                ("object_or_class", false, false),
                ("class", false, false),
                ("allow_string", true, false),
            ],
            Self::InterfaceExists => &[("interface", false, false), ("autoload", true, false)],
            Self::TraitExists => &[("trait", false, false), ("autoload", true, false)],
            Self::MethodExists => &[("object_or_class", false, false), ("method", false, false)],
            Self::PropertyExists => &[
                ("object_or_class", false, false),
                ("property", false, false),
            ],
            Self::Exit => &[("status", true, false)],
            Self::GetIncludedFiles
            | Self::FuncNumArgs
            | Self::FuncGetArgs
            | Self::GetDefinedVars => &[],
            Self::FuncGetArg => &[("position", false, false)],
            Self::AutoloadRegister => &[
                ("callback", true, false),
                ("throw", true, false),
                ("prepend", true, false),
            ],
            Self::AutoloadUnregister => &[("callback", false, false)],
            Self::AutoloadFunctions => &[],
            Self::AutoloadCall => &[("class", false, false)],
            Self::Autoload => &[("class", false, false), ("file_extensions", true, false)],
            Self::AutoloadExtensions => &[("file_extensions", true, false)],
            Self::ErrorReporting => &[("error_level", true, false)],
            Self::Strlen | Self::StrRev | Self::StrLower | Self::StrUpper | Self::Bin2Hex => {
                &[("string", false, false)]
            }
            Self::Trim(..) => &[("string", false, false), ("characters", true, false)],
            Self::SlashEscape(true, true) => {
                &[("string", false, false), ("characters", false, false)]
            }
            Self::SlashEscape(..) => &[("string", false, false)],
            Self::StripTags => &[("string", false, false), ("allowed_tags", true, false)],
            Self::Hex2Bin | Self::FirstCase(_) => &[("string", false, false)],
            Self::Ucwords => &[("string", false, false), ("separators", true, false)],
            Self::Nl2Br => &[("string", false, false), ("use_xhtml", true, false)],
            Self::CountChars => &[("string", false, false), ("mode", true, false)],
            Self::StrPad => &[
                ("string", false, false),
                ("length", false, false),
                ("pad_string", true, false),
                ("pad_type", true, false),
            ],
            Self::StrPbrk => &[("string", false, false), ("characters", false, false)],
            Self::StrTranslate => &[
                ("string", false, false),
                ("from", false, false),
                ("to", true, false),
            ],
            Self::StringSearch(kind) => kind.parameters(),
            Self::StrCompare(_, false) => &[("string1", false, false), ("string2", false, false)],
            Self::StrCompare(_, true) => &[
                ("string1", false, false),
                ("string2", false, false),
                ("length", false, false),
            ],
            Self::SubstrCompare => &[
                ("haystack", false, false),
                ("needle", false, false),
                ("offset", false, false),
                ("length", true, false),
                ("case_insensitive", true, false),
            ],
            Self::SubstrCount => &[
                ("haystack", false, false),
                ("needle", false, false),
                ("offset", true, false),
                ("length", true, false),
            ],
            Self::StrSpan(_) => &[
                ("string", false, false),
                ("characters", false, false),
                ("offset", true, false),
                ("length", true, false),
            ],
            Self::Explode => &[
                ("separator", false, false),
                ("string", false, false),
                ("limit", true, false),
            ],
            Self::Implode(_) => &[("separator", false, false), ("array", true, false)],
            Self::StrSplit => &[("string", false, false), ("length", true, false)],
            Self::ChunkSplit => &[
                ("string", false, false),
                ("length", true, false),
                ("separator", true, false),
            ],
            Self::StrReplace(_) => &[
                ("search", false, false),
                ("replace", false, false),
                ("subject", false, false),
                ("count", true, false),
            ],
            Self::Substr => &[
                ("string", false, false),
                ("offset", false, false),
                ("length", true, false),
            ],
            Self::SubstrReplace => &[
                ("string", false, false),
                ("replace", false, false),
                ("offset", false, false),
                ("length", true, false),
            ],
            Self::StrRepeat => &[("string", false, false), ("times", false, false)],
            Self::Chr => &[("codepoint", false, false)],
            Self::Ord => &[("character", false, false)],
            Self::Count => &[("value", false, false), ("mode", true, false)],
            Self::VarDump => &[("value", false, false), ("values", false, true)],
            Self::Printf | Self::Sprintf => &[("format", false, false), ("values", false, true)],
            Self::Vprintf | Self::Vsprintf => &[("format", false, false), ("values", false, false)],
            Self::PrintR | Self::VarExport => &[("value", false, false), ("return", true, false)],
            Self::GetType | Self::GetDebugType | Self::Is(_) => &[("value", false, false)],
            Self::ArrayKeys => &[
                ("array", false, false),
                ("filter_value", true, false),
                ("strict", true, false),
            ],
            Self::ArrayValues => &[("array", false, false)],
            Self::ArrayNumeric(_) => &[("array", false, false)],
            Self::ArrayEdge(..) | Self::ArrayIsList | Self::ArrayFlip | Self::ArrayCountValues => {
                &[("array", false, false)]
            }
            Self::ArrayCombine => &[("keys", false, false), ("values", false, false)],
            Self::ArrayMerge(_) => &[("arrays", false, true)],
            Self::ArrayReplace(_) => &[("array", false, false), ("replacements", false, true)],
            Self::ArrayCursor(_) => &[("array", false, false)],
            Self::ArraySort(sorting::Sort::Natural | sorting::Sort::NaturalCase) => {
                &[("array", false, false)]
            }
            Self::ArraySort(_) => &[("array", false, false), ("flags", true, false)],
            Self::Compact => &[("var_name", false, false), ("var_names", false, true)],
            Self::Extract => &[
                ("array", false, false),
                ("flags", true, false),
                ("prefix", true, false),
            ],
            Self::ArrayMultisort => &[("array", false, false), ("rest", false, true)],
            Self::ArrayUserSort(_) => &[("array", false, false), ("callback", false, false)],
            Self::MinMax(_) => &[("value", false, false), ("values", false, true)],
            Self::ArrayWalk(_) => &[
                ("array", false, false),
                ("callback", false, false),
                ("arg", true, false),
            ],
            Self::Range => &[
                ("start", false, false),
                ("end", false, false),
                ("step", true, false),
            ],
            Self::Math(kind) => kind.parameters(),
            Self::Round => &[
                ("num", false, false),
                ("precision", true, false),
                ("mode", true, false),
            ],
            Self::NumberFormat => &[
                ("num", false, false),
                ("decimals", true, false),
                ("decimal_separator", true, false),
                ("thousands_separator", true, false),
            ],
            Self::BaseConvert => &[
                ("num", false, false),
                ("from_base", false, false),
                ("to_base", false, false),
            ],
            Self::BaseDecode(2) => &[("binary_string", false, false)],
            Self::BaseDecode(8) => &[("octal_string", false, false)],
            Self::BaseDecode(_) => &[("hex_string", false, false)],
            Self::BaseEncode(_) => &[("num", false, false)],
            Self::ArraySlice => &[
                ("array", false, false),
                ("offset", false, false),
                ("length", true, false),
                ("preserve_keys", true, false),
            ],
            Self::ArraySplice => &[
                ("array", false, false),
                ("offset", false, false),
                ("length", true, false),
                ("replacement", true, false),
            ],
            Self::ArrayColumn => &[
                ("array", false, false),
                ("column_key", false, false),
                ("index_key", true, false),
            ],
            Self::ArraySet(_) => &[("array", false, false), ("arrays", false, true)],
            Self::ArrayUserSet(_) => &[("array", false, false), ("rest", false, true)],
            Self::ArrayStack(array_stack::Action::Push | array_stack::Action::Unshift) => {
                &[("array", false, false), ("values", false, true)]
            }
            Self::ArrayStack(_) => &[("array", false, false)],
            Self::ArrayReverse => &[("array", false, false), ("preserve_keys", true, false)],
            Self::ArrayChunk => &[
                ("array", false, false),
                ("length", false, false),
                ("preserve_keys", true, false),
            ],
            Self::ArrayFill => &[
                ("start_index", false, false),
                ("count", false, false),
                ("value", false, false),
            ],
            Self::ArrayFillKeys => &[("keys", false, false), ("value", false, false)],
            Self::ArrayPad => &[
                ("array", false, false),
                ("length", false, false),
                ("value", false, false),
            ],
            Self::ArrayMap => &[
                ("callback", false, false),
                ("array", false, false),
                ("arrays", false, true),
            ],
            Self::ArrayFilter => &[
                ("array", false, false),
                ("callback", true, false),
                ("mode", true, false),
            ],
            Self::ArrayReduce => &[
                ("array", false, false),
                ("callback", false, false),
                ("initial", true, false),
            ],
            Self::ArrayKeyExists => &[("key", false, false), ("array", false, false)],
            Self::ArraySearch(_) => &[
                ("needle", false, false),
                ("haystack", false, false),
                ("strict", true, false),
            ],
            Self::CallUserFunc => &[("callback", false, false), ("args", false, true)],
            Self::CallUserFuncArray => &[("callback", false, false), ("args", false, false)],
            Self::DebugBacktrace | Self::DebugPrintBacktrace => {
                &[("options", true, false), ("limit", true, false)]
            }
            Self::IsCallable => &[
                ("value", false, false),
                ("syntax_only", true, false),
                ("callable_name", true, false),
            ],
            Self::FunctionExists => &[("function", false, false)],
            Self::Define => &[
                ("constant_name", false, false),
                ("value", false, false),
                ("case_insensitive", true, false),
            ],
            Self::Defined | Self::Constant => &[("constant_name", false, false)],
        };
        definitions
            .iter()
            .map(|(name, optional, variadic)| Parameter {
                attributes: Vec::new(),
                by_reference: matches!(self, Self::ArrayMultisort)
                    || (matches!(self, Self::IsCallable) && *name == "callable_name")
                    || (matches!(self, Self::StrReplace(_)) && *name == "count")
                    || matches!(self, Self::ArrayCursor(action) if action.moves())
                    || (matches!(
                        self,
                        Self::ArrayWalk(_)
                            | Self::ArraySort(_)
                            | Self::ArrayUserSort(_)
                            | Self::Extract
                            | Self::ArraySplice
                            | Self::ArrayStack(_)
                    ) && *name == "array"),
                promotion: None,
                name: name.as_bytes().to_vec(),
                ty: None,
                implicit_nullable: false,
                default: optional.then(|| {
                    if matches!((self, *name), (Self::Round, "mode")) {
                        return rounding::default_mode();
                    }
                    if matches!((self, *name), (Self::Math(math::Math::Log), "base")) {
                        return Expr {
                            kind: ExprKind::Constant(b"M_E".to_vec()),
                            line: 0,
                        };
                    }
                    Expr {
                        kind: ExprKind::Literal(match (self, *name) {
                            (Self::Ucwords, "separators") => {
                                Value::String(b" \t\r\n\x0c\x0b".to_vec().into())
                            }
                            (Self::Nl2Br, "use_xhtml") => Value::Bool(true),
                            (Self::Range, "step") => Value::Int(1),
                            (Self::Fopen, "use_include_path") => Value::Bool(false),
                            (Self::Stream(streams::Operation::Seek), "whence") => Value::Int(0),
                            (Self::ArraySort(_) | Self::Extract, "flags") => Value::Int(0),
                            (Self::Extract, "prefix") => Value::String(Vec::new().into()),
                            (Self::ArraySplice, "replacement") => {
                                Value::Array(Rc::new(PhpArray::default()))
                            }
                            (Self::Round, "precision") => Value::Int(0),
                            (Self::ArraySearch(_), "strict") => Value::Bool(false),
                            (Self::NumberFormat, "decimals") => Value::Int(0),
                            (Self::NumberFormat, "decimal_separator") => {
                                Value::String(b".".to_vec().into())
                            }
                            (Self::NumberFormat, "thousands_separator") => {
                                Value::String(b",".to_vec().into())
                            }
                            (
                                Self::ArraySlice | Self::ArrayReverse | Self::ArrayChunk,
                                "preserve_keys",
                            ) => Value::Bool(false),
                            (Self::CountChars, "mode") => Value::Int(0),
                            (Self::StrPad, "pad_string") => Value::String(b" ".to_vec().into()),
                            (Self::StrPad, "pad_type") => Value::Int(1),
                            (Self::Explode, "limit") => Value::Int(i64::MAX),
                            (Self::StrSplit, "length") => Value::Int(1),
                            (Self::ChunkSplit, "length") => Value::Int(76),
                            (Self::ChunkSplit, "separator") => {
                                Value::String(b"\r\n".to_vec().into())
                            }
                            (Self::StringSearch(_), "offset") => Value::Int(0),
                            (Self::SubstrCount | Self::StrSpan(_), "offset") => Value::Int(0),
                            (Self::SubstrCompare, "case_insensitive") => Value::Bool(false),
                            (Self::StringSearch(_), "before_needle") => Value::Bool(false),
                            (Self::Trim(..), "characters") => {
                                Value::String(b" \n\r\t\x0b\0".to_vec().into())
                            }
                            (Self::IsCallable, "syntax_only") => Value::Bool(false),
                            (Self::DebugBacktrace, "options") => Value::Int(1),
                            (Self::DebugBacktrace | Self::DebugPrintBacktrace, _) => Value::Int(0),
                            _ => Value::Null,
                        }),
                        line: 0,
                    }
                }),
                variadic: *variadic,
            })
            .collect()
    }
}
impl Runtime {
    pub(super) fn call_builtin(
        &mut self,
        builtin: Builtin,
        name: &[u8],
        bound: functions::BoundArguments,
        line: usize,
        dynamic: bool,
        result_used: bool,
    ) -> Result<Value, Error> {
        let functions::BoundArguments {
            fixed: args,
            references,
            extra,
            direct_positional,
            ..
        } = bound;
        if matches!(
            builtin,
            Builtin::AutoloadRegister
                | Builtin::AutoloadUnregister
                | Builtin::AutoloadFunctions
                | Builtin::AutoloadCall
                | Builtin::Autoload
                | Builtin::AutoloadExtensions
        ) {
            return self.autoload_builtin(builtin, &args, line);
        }
        if matches!(builtin, Builtin::ErrorReporting) {
            let previous = self.diagnostics.reporting;
            if let Some(value) = args[0]
                .as_ref()
                .filter(|value| !matches!(value, Value::Null))
            {
                self.diagnostics.reporting = self.native_int(value, "error_reporting", 1, line).map_err(|error| {
                    if error.kind != ErrorKind::TypeError { return error; }
                    let given = match value {
                        Value::Bool(true) => "true",
                        Value::Bool(false) => "false",
                        _ => debug_type(value),
                    };
                    Error::new(ErrorKind::TypeError, format!("error_reporting(): Argument #1 ($error_level) must be of type ?int, {given} given"), line)
                })? as i32 as i64;
            }
            return Ok(Value::Int(previous));
        }
        if matches!(
            builtin,
            Builtin::GetClass | Builtin::GetParentClass | Builtin::GetCalledClass
        ) {
            return self.inspect_class(builtin, args.first().and_then(Option::as_ref), line);
        }
        if matches!(builtin, Builtin::Exit) {
            return self.terminate(args[0].as_ref().unwrap_or(&Value::Int(0)), line);
        }
        if matches!(
            builtin,
            Builtin::FuncNumArgs | Builtin::FuncGetArg | Builtin::FuncGetArgs
        ) {
            return self.inspect_arguments(
                builtin,
                args.first().and_then(Option::as_ref),
                dynamic,
                line,
            );
        }
        if matches!(builtin, Builtin::GetDefinedVars) {
            return self.defined_variables(dynamic, line);
        }
        if matches!(builtin, Builtin::Compact) {
            return self.compact_variables(&args, &extra, dynamic, line);
        }
        if matches!(builtin, Builtin::GetIncludedFiles) {
            let mut paths = PhpArray::default();
            for path in &self.included_files {
                paths.append(
                    Value::String(path.as_os_str().as_encoded_bytes().to_vec().into()),
                    line,
                )?;
            }
            return Ok(Value::Array(Rc::new(paths)));
        }
        if matches!(
            builtin,
            Builtin::DebugBacktrace | Builtin::DebugPrintBacktrace
        ) {
            return self.debug_backtrace(builtin, &args, line);
        }
        let name = String::from_utf8_lossy(name);
        if let Builtin::Math(kind) = builtin {
            return self.math(kind, &name, &args, line);
        }
        if matches!(builtin, Builtin::ArrayMerge(_) | Builtin::ArrayReplace(_)) {
            return self.array_join(builtin, &name, &args, &extra, line);
        }
        if let Builtin::ArraySet(operation) = builtin {
            return self.array_set(operation, &name, args[0].as_ref().unwrap(), &extra, line);
        }
        if let Builtin::ArrayUserSet(operation) = builtin {
            return self.array_user_set(operation, &name, args[0].as_ref().unwrap(), &extra, line);
        }
        if let Builtin::ArrayCursor(action) = builtin {
            return self.array_cursor(action, &name, args, references, line, result_used);
        }
        if matches!(builtin, Builtin::Extract) {
            return self.extract_variables(args, references, dynamic, line);
        }
        if matches!(builtin, Builtin::ArraySplice) {
            return self.array_splice(args, references, line, result_used);
        }
        if let Builtin::ArrayStack(action) = builtin {
            return self.array_stack(action, &name, args, references, extra, line);
        }
        if matches!(builtin, Builtin::ArrayMultisort) {
            return self.array_multisort(args, references, extra, line);
        }
        if let Builtin::ArrayUserSort(kind) = builtin {
            return self.array_user_sort(kind, &name, args, references, line);
        }
        if let Builtin::ArraySort(kind) = builtin {
            return self.array_sort(kind, &name, args, references, line);
        }
        if let Builtin::ArrayWalk(recursive) = builtin {
            return self.array_walk(recursive, &name, args, references, line);
        }
        let first = args[0].as_ref().unwrap(); // Required arguments checked by common binder.
        match builtin {
            Builtin::Fopen => self.fopen(&args, line),
            Builtin::FileExists => self.file_path_operation(false, &args, line),
            Builtin::Unlink => self.file_path_operation(true, &args, line),
            Builtin::Stream(operation) => self.stream_operation(operation, &name, &args, line),
            Builtin::ResourceInfo(id) => {
                let Value::Resource(resource) = first else {
                    return Err(type_error(first, &name, 1, "resource", line));
                };
                Ok(if id {
                    Value::Int(resource.id)
                } else {
                    Value::String(resource.type_name().as_bytes().to_vec().into())
                })
            }
            Builtin::ArrayColumn => self.array_column(&args, line),
            Builtin::MinMax(maximum) => {
                self.extremum(maximum, &name, first, &extra, direct_positional, line)
            }
            Builtin::StrReplace(insensitive) => {
                self.string_replace(insensitive, &args, &references, line)
            }
            Builtin::Substr => self.substring(&args, line),
            Builtin::SubstrReplace => self.substring_replace(&args, line),
            Builtin::StringSearch(kind) => self.string_search(kind, &args, line),
            Builtin::StrCompare(insensitive, bounded) => {
                self.string_compare(insensitive, bounded, &args, line)
            }
            Builtin::SubstrCompare => self.substring_compare(&args, line),
            Builtin::SubstrCount => self.substring_count(&args, line),
            Builtin::StrSpan(included) => self.string_span(included, &args, line),
            Builtin::Explode => self.explode_string(&args, line),
            Builtin::Implode(name) => self.implode_strings(name, direct_positional, &args, line),
            Builtin::StrSplit | Builtin::ChunkSplit => self.split_chunks(builtin, &args, line),
            Builtin::StrTranslate => self.translate_string(&args, line),
            Builtin::SlashEscape(add, c_style) => self.escape_slashes(add, c_style, &args, line),
            Builtin::StripTags => self.strip_tags(&args, line),
            Builtin::Hex2Bin
            | Builtin::FirstCase(_)
            | Builtin::Ucwords
            | Builtin::Nl2Br
            | Builtin::CountChars
            | Builtin::StrPad
            | Builtin::StrPbrk => self.byte_string_builtin(builtin, &args, line),
            Builtin::ErrorReporting
            | Builtin::AutoloadRegister
            | Builtin::AutoloadUnregister
            | Builtin::AutoloadFunctions
            | Builtin::AutoloadCall
            | Builtin::Autoload
            | Builtin::AutoloadExtensions => unreachable!(),
            Builtin::Clone => self.clone_value(first, args[1].as_ref(), line),
            Builtin::StrRepeat
            | Builtin::StrRev
            | Builtin::Chr
            | Builtin::Ord
            | Builtin::StrLower
            | Builtin::StrUpper
            | Builtin::Bin2Hex
            | Builtin::Trim(..) => self.string_builtin(builtin, &args, line),
            Builtin::ClassAlias => self.class_alias(&args, line),
            Builtin::IsA | Builtin::IsSubclassOf => self.inspect_inheritance(builtin, &args, line),
            Builtin::GetClass | Builtin::GetParentClass | Builtin::GetCalledClass => unreachable!(),
            Builtin::GetClassVars | Builtin::GetClassMethods => {
                self.class_inspection(builtin, first, line)
            }
            Builtin::GetObjectVars => match first {
                Value::Object(object) => Ok(Value::Array(Rc::new(self.object_variables(object)))),
                Value::Closure(_) => Ok(Value::Array(Rc::new(PhpArray::default()))),
                _ => Err(Error::new(
                    ErrorKind::TypeError,
                    "get_object_vars(): Argument #1 ($object) must be of type object",
                    line,
                )),
            },
            Builtin::ClassExists
            | Builtin::InterfaceExists
            | Builtin::TraitExists
            | Builtin::EnumExists => {
                let class = self.native_string(first, &name, 1, line)?;
                let autoload = args[1]
                    .as_ref()
                    .map(|value| self.native_bool(value, &name, 2, line))
                    .transpose()?
                    .unwrap_or(true);
                let found = if autoload {
                    self.load_class(&class, line)?
                } else {
                    self.find_class(&class)
                };
                Ok(Value::Bool(
                    found.is_some_and(|c| {
                        (!matches!(builtin, Builtin::EnumExists) || c.declaration.is_enum)
                            && c.declaration.is_interface
                                == matches!(builtin, Builtin::InterfaceExists)
                            && c.declaration.is_trait == matches!(builtin, Builtin::TraitExists)
                    }) || matches!(builtin, Builtin::ClassExists)
                        && class
                            .strip_prefix(b"\\")
                            .unwrap_or(&class)
                            .eq_ignore_ascii_case(b"Closure"),
                ))
            }
            Builtin::MethodExists | Builtin::PropertyExists => {
                let member = self.native_string(args[1].as_ref().unwrap(), &name, 2, line)?;
                let class = match first {
                    Value::Object(object) => Some(object.class.clone()),
                    Value::String(name) => self.load_class(&name.bytes(), line)?,
                    Value::Closure(_) => None,
                    _ => {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            format!(
                                "{name}(): Argument #1 ($object_or_class) must be of type object|string"
                            ),
                            line,
                        ));
                    }
                };
                let exists = if matches!(builtin, Builtin::MethodExists) {
                    class.is_some_and(|c| {
                        objects::method(&c, &member).is_some_and(|m| {
                            matches!(first, Value::Object(_))
                                || m.visibility != crate::ast::Visibility::Private
                                || method_visibility::declaring_name(&m)
                                    .eq_ignore_ascii_case(&c.name)
                        })
                    })
                } else {
                    class.is_some_and(|c| {
                        c.property_owner(&member).is_some()
                            || c.static_owner(&member).is_some_and(|owner| {
                                owner.name == c.name
                                    || owner.declaration.static_properties.iter().any(|p| {
                                        p.name == member
                                            && p.visibility != crate::ast::Visibility::Private
                                    })
                            })
                    }) || matches!(first, Value::Object(o) if self.check_instance_property_access(o, &member, true, line)? && o.properties.borrow().get(&self.instance_property_key(o, &member)).is_some())
                };
                Ok(Value::Bool(exists))
            }
            Builtin::Range => self.range(&args, line),
            Builtin::Round => self.round(&args, line),
            Builtin::NumberFormat => self.number_format(&args, line),
            Builtin::BaseConvert | Builtin::BaseDecode(_) | Builtin::BaseEncode(_) => {
                self.base_conversion(builtin, &name, &args, line)
            }
            Builtin::ArraySlice
            | Builtin::ArrayReverse
            | Builtin::ArrayChunk
            | Builtin::ArrayFill
            | Builtin::ArrayFillKeys
            | Builtin::ArrayPad => self.array_segments(builtin, &name, &args, line),
            Builtin::ArrayMap => self.array_map(&args, &extra, line),
            Builtin::CallUserFunc | Builtin::CallUserFuncArray => {
                self.call_user_function(builtin, &args, &extra, line)
            }
            Builtin::ArrayFilter => self.array_filter(&args, line),
            Builtin::ArrayReduce => self.array_reduce(&args, line),
            Builtin::Exit
            | Builtin::Math(_)
            | Builtin::GetIncludedFiles
            | Builtin::GetDefinedVars
            | Builtin::Compact
            | Builtin::FuncNumArgs
            | Builtin::FuncGetArg
            | Builtin::FuncGetArgs => unreachable!(),
            Builtin::Define => {
                let constant = self.native_string(first, &name, 1, line)?;
                if constant.windows(2).any(|part| part == b"::") {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "define(): Argument #1 ($constant_name) cannot be a class constant",
                        line,
                    ));
                }
                if let Some(flag) = &args[2]
                    && self.native_bool(flag, &name, 3, line)?
                {
                    self.diagnostics.push(format!("define(): Argument #3 ($case_insensitive) is ignored since declaration of case-insensitive constants is no longer supported on line {line}"));
                }
                Ok(Value::Bool(self.define_constant(
                    constant,
                    args[1].as_ref().unwrap().clone(),
                    line,
                )))
            }
            Builtin::Defined | Builtin::Constant => {
                let constant = self.native_string(first, &name, 1, line)?;
                if constant.windows(2).any(|p| p == b"::") {
                    self.named_class_constant(&constant, matches!(builtin, Builtin::Defined), line)
                } else if matches!(builtin, Builtin::Defined) {
                    Ok(Value::Bool(self.lookup_constant(&constant).is_some()))
                } else {
                    self.read_constant(&constant, line)
                }
            }
            Builtin::Strlen => Ok(Value::Int(
                self.native_string(first, &name, 1, line)?.len() as i64
            )),
            Builtin::ObjectId | Builtin::ObjectHash => {
                let id = match first {
                    Value::Object(object) => object.object_id,
                    Value::Closure(closure) => closure.object_id,
                    _ => return Err(type_error(first, &name, 1, "object", line)),
                };
                Ok(if matches!(builtin, Builtin::ObjectId) {
                    Value::Int(id as i64)
                } else {
                    Value::String(format!("{id:016x}0000000000000000").into_bytes().into())
                })
            }
            Builtin::GetType => Ok(Value::String(
                match first {
                    Value::Reference(_) | Value::Indirect(_) => {
                        unreachable!("arguments are dereferenced")
                    }
                    Value::Null => "NULL",
                    Value::Bool(_) => "boolean",
                    Value::Int(_) => "integer",
                    Value::Float(_) => "double",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Closure(_) | Value::Object(_) => "object",
                    Value::Resource(resource) => {
                        if resource.is_open() {
                            "resource"
                        } else {
                            "resource (closed)"
                        }
                    }
                }
                .as_bytes()
                .to_vec()
                .into(),
            )),
            Builtin::GetDebugType => {
                let kind = if let Value::Resource(resource) = first {
                    resource.debug_type()
                } else {
                    debug_type(first)
                };
                Ok(Value::String(kind.as_bytes().to_vec().into()))
            }
            Builtin::Is(kind) => Ok(Value::Bool(
                (debug_type(first) == kind
                    && !matches!(first, Value::Resource(resource) if !resource.is_open()))
                    || (kind == "object" && matches!(first, Value::Closure(_) | Value::Object(_))),
            )),
            Builtin::PrintR => self.print_r(&args, line),
            Builtin::VarExport => self.var_export(&args, line),
            Builtin::IsCallable => self.inspect_callable(&args, &references, line),
            Builtin::DebugBacktrace | Builtin::DebugPrintBacktrace => unreachable!(),
            Builtin::Printf | Builtin::Sprintf | Builtin::Vprintf | Builtin::Vsprintf => {
                self.formatted_print(builtin, &name, &args, &extra, line)
            }
            Builtin::VarDump => {
                if extra
                    .entries()
                    .iter()
                    .any(|(k, _)| matches!(k, ArrayKey::String(_)))
                {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        "var_dump() does not accept unknown named parameters",
                        line,
                    ));
                }
                dump(first, 0, &mut self.output, line, &mut self.diagnostics);
                for (_, value) in extra.entries() {
                    dump(value, 0, &mut self.output, line, &mut self.diagnostics);
                }
                self.diagnostics.output_position = self.output.len();
                Ok(Value::Null)
            }
            Builtin::Count => {
                let array = require_array(first, &name, 1, line)?;
                let mode = match &args[1] {
                    None => 0,
                    Some(v) => self.native_int(v, &name, 2, line)?,
                };
                if mode != 0 && mode != 1 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!(
                            "{name}(): Argument #2 ($mode) must be either COUNT_NORMAL or COUNT_RECURSIVE"
                        ),
                        line,
                    ));
                }
                Ok(Value::Int(count(
                    array,
                    mode == 1,
                    &mut HashSet::new(),
                    line,
                    &mut self.diagnostics,
                )))
            }
            Builtin::ArrayValues => {
                let array = require_array(first, &name, 1, line)?;
                let mut result = PhpArray::default();
                for (_, v) in array.entries() {
                    result.append(v.clone(), line)?;
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayEdge(..)
            | Builtin::ArrayIsList
            | Builtin::ArrayFlip
            | Builtin::ArrayCountValues
            | Builtin::ArrayCombine => self.array_inspection(builtin, &name, &args, line),
            Builtin::ArrayMerge(_)
            | Builtin::ArrayReplace(_)
            | Builtin::ArraySet(_)
            | Builtin::ArrayUserSet(_) => {
                unreachable!()
            }
            Builtin::ArrayCursor(_) => unreachable!(),
            Builtin::ArrayWalk(_)
            | Builtin::ArraySort(_)
            | Builtin::ArrayUserSort(_)
            | Builtin::ArrayMultisort
            | Builtin::Extract
            | Builtin::ArraySplice
            | Builtin::ArrayStack(_) => {
                unreachable!()
            }
            Builtin::ArrayKeys => {
                if args[1].is_none() && args[2].is_some() {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        "array_keys(): Argument #2 ($filter_value) must be passed explicitly, because the default value is not known",
                        line,
                    ));
                }
                let array = require_array(first, &name, 1, line)?;
                let strict = match &args[2] {
                    None => false,
                    Some(v) => self.native_bool(v, &name, 3, line)?,
                };
                let mut result = PhpArray::default();
                for (key, value) in array.entries() {
                    let selected = match &args[1] {
                        None => true,
                        Some(filter) if strict => value.identical(filter, line)?,
                        Some(filter) => self.compare(value, filter, line)? == Some(Ordering::Equal),
                    };
                    if selected {
                        result.append(key.to_value(), line)?;
                    }
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayKeyExists => {
                let array = require_array(args[1].as_ref().unwrap(), &name, 2, line)?;
                let key = ArrayKey::from_value(first, line, &mut self.diagnostics)?;
                Ok(Value::Bool(array.get(&key).is_some()))
            }
            Builtin::ArraySearch(return_key) => {
                let array = require_array(args[1].as_ref().unwrap(), &name, 2, line)?;
                let strict = args[2]
                    .as_ref()
                    .map_or(Ok(false), |value| self.native_bool(value, &name, 3, line))?;
                for (key, value) in array.entries() {
                    let matches = if strict {
                        first.identical(value, line)?
                    } else {
                        self.compare(first, value, line)? == Some(Ordering::Equal)
                    };
                    if matches {
                        return Ok(if return_key {
                            key.to_value()
                        } else {
                            Value::Bool(true)
                        });
                    }
                }
                Ok(Value::Bool(false))
            }
            Builtin::ArrayNumeric(product) => {
                let array = require_array(first, &name, 1, line)?;
                let op = if product { "*" } else { "+" };
                let operation = if product {
                    "Multiplication"
                } else {
                    "Addition"
                };
                let mut total = Value::Int(i64::from(product));
                for (_, value) in array.entries() {
                    match binary(
                        op,
                        total.clone(),
                        value.clone(),
                        line,
                        &mut self.diagnostics,
                    ) {
                        Ok(result) => total = result,
                        Err(error) if error.kind == ErrorKind::TypeError => {
                            // PHP keeps its historical zero fallback for direct
                            // nonnumeric strings, while other invalid entries
                            // (including references to those strings) are skipped.
                            if matches!(value, Value::String(_)) {
                                total =
                                    binary(op, total, Value::Int(0), line, &mut self.diagnostics)?;
                            }
                            self.diagnostics.push(format!(
                                "{name}(): {operation} is not supported on type {} on line {line}",
                                debug_type(&value.dereferenced())
                            ));
                        }
                        Err(error) => return Err(error),
                    }
                }
                Ok(total)
            }
            Builtin::FunctionExists => {
                let bytes = self.native_string(first, &name, 1, line)?;
                let name = bytes
                    .strip_prefix(b"\\")
                    .unwrap_or(&bytes)
                    .to_ascii_lowercase();
                Ok(Value::Bool(
                    self.functions.contains_key(&name) || Builtin::lookup(&name).is_some(),
                ))
            }
        }
    }
    pub(super) fn native_string(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        self.native_string_type(value, function, position, line, "string")
    }

    pub(super) fn native_string_type(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
        kind: &str,
    ) -> Result<Vec<u8>, Error> {
        if !self.strict_types && self.can_stringify(value) {
            return self.string_bytes(value, line);
        }
        if matches!(
            value,
            Value::Array(_) | Value::Closure(_) | Value::Object(_) | Value::Resource(_)
        ) || (self.strict_types && !matches!(value, Value::String(_)))
        {
            return Err(type_error(value, function, position, kind, line));
        }
        self.warn_null(value, function, position, kind, line);
        value.warn_nan("string", line, &mut self.diagnostics);
        Ok(value.to_bytes())
    }

    pub(super) fn native_array_or_string(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<Value, Error> {
        self.native_array_or_string_type(value, function, position, line, "array|string")
    }

    pub(super) fn native_array_or_string_type(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
        kind: &str,
    ) -> Result<Value, Error> {
        if matches!(value, Value::Array(_) | Value::String(_)) {
            return Ok(value.clone());
        }
        if self.strict_types
            || matches!(value, Value::Resource(_))
            || (matches!(value, Value::Object(_) | Value::Closure(_)) && !self.can_stringify(value))
        {
            return Err(type_error(value, function, position, kind, line));
        }
        self.warn_null(value, function, position, kind, line);
        Ok(Value::String(self.string_bytes(value, line)?.into()))
    }
    pub(super) fn native_int(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<i64, Error> {
        self.native_int_type(value, function, position, line, "int")
    }

    pub(super) fn native_int_type(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
        kind: &str,
    ) -> Result<i64, Error> {
        if (self.strict_types && !matches!(value, Value::Int(_)))
            || matches!(
                value,
                Value::Array(_) | Value::Closure(_) | Value::Object(_) | Value::Resource(_)
            )
            || matches!(value, Value::String(s) if crate::value::numeric_string(&s.bytes()).is_none())
        {
            return Err(type_error(value, function, position, kind, line));
        }
        let number = match value {
            Value::String(s) => crate::value::numeric_string(&s.bytes()).unwrap(),
            _ => value.clone(),
        };
        if let Value::Float(n) = number
            && (!n.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&n))
        {
            return Err(type_error(value, function, position, kind, line));
        }
        if let (Value::String(bytes), Value::Float(n)) = (value, &number) {
            let converted = *n as i64;
            if *n != converted as f64 {
                self.diagnostics.push(format!(
                    "Deprecated: Implicit conversion from float-string \"{}\" to int loses precision on line {line}",
                    String::from_utf8_lossy(&bytes.bytes())
                ));
            }
            return Ok(converted);
        }
        self.warn_null(value, function, position, kind, line);
        value.integer(line, &mut self.diagnostics)
    }
    pub(super) fn native_bool(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<bool, Error> {
        if matches!(
            value,
            Value::Array(_) | Value::Closure(_) | Value::Object(_) | Value::Resource(_)
        ) || (self.strict_types && !matches!(value, Value::Bool(_)))
        {
            return Err(type_error(value, function, position, "bool", line));
        }
        self.warn_null(value, function, position, "bool", line);
        Ok(value.to_bool(line, &mut self.diagnostics))
    }
    pub(super) fn warn_null(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        kind: &str,
        line: usize,
    ) {
        if matches!(value, Value::Null) {
            self.diagnostics.push(format!("Deprecated: {function}(): Passing null to parameter #{position}{} of type {kind} is deprecated on line {line}", parameter_label(function, position)));
        }
    }
}
fn parameter_label(function: &str, position: usize) -> String {
    Builtin::lookup(function.to_ascii_lowercase().as_bytes())
        .and_then(|builtin| builtin.parameters().get(position - 1).cloned())
        .filter(|parameter| !parameter.variadic)
        .map_or_else(String::new, |p| {
            format!(" (${})", String::from_utf8_lossy(&p.name))
        })
}
pub(super) fn type_error(
    value: &Value,
    function: &str,
    position: usize,
    kind: &str,
    line: usize,
) -> Error {
    let given = match value {
        Value::Bool(true) => "true",
        Value::Bool(false) => "false",
        _ => debug_type(value),
    };
    Error::new(
        ErrorKind::TypeError,
        format!(
            "{function}(): Argument #{position}{} must be of type {kind}, {given} given",
            parameter_label(function, position)
        ),
        line,
    )
}
pub(super) fn require_array<'a>(
    value: &'a Value,
    function: &str,
    position: usize,
    line: usize,
) -> Result<&'a PhpArray, Error> {
    match value {
        Value::Array(array) => Ok(array),
        _ => Err(type_error(value, function, position, "array", line)),
    }
}
pub(super) fn debug_type(value: &Value) -> &str {
    match value {
        Value::Reference(_) | Value::Indirect(_) => {
            unreachable!("values are dereferenced before type checks")
        }
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Closure(_) => "Closure",
        Value::Object(object) => object.class_name.split('\0').next().unwrap(),
        Value::Resource(_) => "resource",
    }
}
fn count(
    array: &PhpArray,
    recursive: bool,
    seen: &mut HashSet<usize>,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> i64 {
    if !recursive {
        return array.len() as i64;
    }
    let identity = array as *const PhpArray as usize;
    if !seen.insert(identity) {
        warnings.push(format!("count(): Recursion detected on line {line}"));
        return 0;
    }
    let mut result = array.len() as i64;
    for (_, value) in array.entries() {
        if let Value::Array(array) = value.dereferenced() {
            result += count(&array, true, seen, line, warnings);
        }
    }
    seen.remove(&identity);
    result
}
fn dump(value: &Value, depth: usize, out: &mut Vec<u8>, line: usize, warnings: &mut Diagnostics) {
    dump_inner(value, depth, out, &mut HashSet::new(), line, warnings);
}
fn dump_inner(
    value: &Value,
    depth: usize,
    out: &mut Vec<u8>,
    seen: &mut HashSet<usize>,
    line: usize,
    warnings: &mut Diagnostics,
) {
    match value {
        Value::Indirect(slot) => dump_inner(&slot.borrow(), depth, out, seen, line, warnings),
        Value::Reference(reference) => {
            let value = reference.borrow();
            let recursive = match &*value {
                Value::Array(array) => seen.contains(&(Rc::as_ptr(array) as usize)),
                Value::Object(object) => seen.contains(&object.object_id),
                _ => false,
            };
            if Rc::strong_count(reference) > 1 && !recursive {
                out.push(b'&');
            }
            dump_inner(&value, depth, out, seen, line, warnings);
        }
        Value::Object(object) => {
            if object.class.declaration.is_enum {
                let name = object
                    .properties
                    .borrow()
                    .get(&ArrayKey::String(b"name".to_vec()))
                    .unwrap()
                    .to_bytes();
                out.extend(b"enum(");
                out.extend(&object.class.name);
                out.extend(b"::");
                out.extend(name);
                out.extend(b")\n");
                return;
            }
            if !seen.insert(object.object_id) {
                out.extend_from_slice(b"*RECURSION*\n");
                return;
            }
            let properties = object.export_properties();
            out.extend_from_slice(
                format!(
                    "object({})#{} ({}) {{\n",
                    object.class_name.split('\0').next().unwrap(),
                    object.object_id,
                    properties.len()
                )
                .as_bytes(),
            );
            let mut keys: Vec<ArrayKey> = object
                .class
                .properties
                .iter()
                .filter(|p| {
                    !object.hides_properties()
                        && (p.ty.is_some()
                            || properties
                                .get(&ArrayKey::String(p.storage_name()))
                                .is_some())
                })
                .map(|p| ArrayKey::String(p.storage_name()))
                .collect();
            for (key, _) in properties.entries() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
            for key in &keys {
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                out.extend_from_slice(b"[\"");
                let raw = key.to_value().to_bytes();
                let decoded = crate::object::property_name(&raw).unwrap_or_else(|message| {
                    warnings.output_position = out.len();
                    warnings.push(format!("Notice: {message} on line {line}"));
                    crate::object::PropertyName {
                        scope: None,
                        name: &raw,
                    }
                });
                if let Some(scope) = decoded.scope {
                    let name = decoded.name.split(|b| *b == 0).next().unwrap();
                    out.extend_from_slice(name);
                    if scope.first() == Some(&b'*') {
                        out.extend_from_slice(b"\":protected]=>\n");
                    } else {
                        out.extend_from_slice(b"\":\"");
                        out.extend_from_slice(scope);
                        out.extend_from_slice(b"\":private]=>\n");
                    }
                } else {
                    out.extend_from_slice(decoded.name);
                    out.extend_from_slice(b"\"]=>\n");
                }
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                if let Some(value) = properties.get(key) {
                    dump_inner(value, depth + 2, out, seen, line, warnings);
                } else {
                    let property = object
                        .class
                        .properties
                        .iter()
                        .find(|p| ArrayKey::String(p.storage_name()) == *key)
                        .expect("typed property slot");
                    let owner = object.class.property_slot_owner(property);
                    let ty = property.ty.as_ref().expect("uninitialized typed property");
                    out.extend(
                        format!(
                            "uninitialized({})\n",
                            typed_properties::type_label(ty, &owner)
                        )
                        .as_bytes(),
                    );
                }
            }
            out.extend(std::iter::repeat_n(b' ', depth));
            out.extend_from_slice(b"}\n");
            seen.remove(&object.object_id);
        }
        Value::Closure(closure) => {
            let properties = closure_debug_properties(closure);
            let mut rendered = Vec::new();
            let first_diagnostic = warnings.events.len();
            dump_inner(
                &Value::Array(Rc::new(properties)),
                depth,
                &mut rendered,
                seen,
                line,
                warnings,
            );
            let start = rendered.iter().position(|b| *b == b'(').unwrap();
            out.extend_from_slice(format!("object(Closure)#{} ", closure.object_id).as_bytes());
            // Nested dumps record positions in the temporary array rendering.
            // Relocate them after replacing that rendering's header.
            for event in &mut warnings.events[first_diagnostic..] {
                event.position = out.len() + event.position.saturating_sub(start);
            }
            out.extend_from_slice(&rendered[start..]);
        }
        Value::Null => out.extend_from_slice(b"NULL\n"),
        Value::Resource(resource) => out.extend_from_slice(
            format!(
                "resource({}) of type ({})\n",
                resource.id,
                resource.type_name()
            )
            .as_bytes(),
        ),
        Value::Bool(b) => out.extend_from_slice(if *b {
            b"bool(true)\n"
        } else {
            b"bool(false)\n"
        }),
        Value::Int(n) => out.extend_from_slice(format!("int({n})\n").as_bytes()),
        Value::Float(n) => out.extend_from_slice(format!("float({})\n", dump_float(*n)).as_bytes()),
        Value::String(s) => {
            out.extend_from_slice(format!("string({}) \"", s.len()).as_bytes());
            out.extend_from_slice(&s.bytes());
            out.extend_from_slice(b"\"\n");
        }
        Value::Array(array) => {
            let identity = Rc::as_ptr(array) as usize;
            if !seen.insert(identity) {
                out.extend_from_slice(b"*RECURSION*\n");
                return;
            }
            out.extend_from_slice(format!("array({}) {{\n", array.len()).as_bytes());
            for (key, value) in array.entries() {
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                match key {
                    ArrayKey::Int(n) => out.extend_from_slice(format!("[{n}]=>\n").as_bytes()),
                    ArrayKey::String(s) => {
                        out.extend_from_slice(b"[\"");
                        out.extend_from_slice(s);
                        out.extend_from_slice(b"\"]=>\n");
                    }
                }
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                dump_inner(value, depth + 2, out, seen, line, warnings);
            }
            out.extend(std::iter::repeat_n(b' ', depth));
            out.extend_from_slice(b"}\n");
            seen.remove(&identity);
        }
    }
}
fn dump_float(n: f64) -> String {
    if n.is_nan() {
        return "NAN".into();
    }
    if n.is_infinite() {
        return if n.is_sign_negative() { "-INF" } else { "INF" }.into();
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let scientific = format!("{n:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..17).contains(&exponent) {
        format!(
            "{}{}E{exponent:+}",
            mantissa,
            if mantissa.contains('.') { "" } else { ".0" }
        )
    } else {
        n.to_string()
    }
}

pub(super) fn closure_debug_properties(closure: &crate::Closure) -> PhpArray {
    let parameters = if closure.magic_name.is_some() {
        vec![functions::magic_parameter()]
    } else {
        closure
            .function
            .as_ref()
            .map(|f| f.parameters.clone())
            .unwrap_or_else(|| Builtin::lookup(&closure.name).unwrap().parameters())
    };
    let mut properties = PhpArray::default();
    let name = if closure.magic_name.is_some() {
        &closure.name
    } else {
        closure.function.as_ref().map_or(&closure.name, |f| &f.name)
    };
    properties.insert(
        ArrayKey::String(b"function".to_vec()),
        Value::String(name.clone().into()),
    );
    if let Some(object) = &closure.this {
        properties.insert(
            ArrayKey::String(b"this".to_vec()),
            Value::Object(object.clone()),
        );
    }
    if !parameters.is_empty() {
        let mut entries = PhpArray::default();
        for param in parameters {
            let mut key = if param.by_reference {
                b"&$".to_vec()
            } else {
                b"$".to_vec()
            };
            key.extend(param.name);
            entries.insert(
                ArrayKey::String(key),
                Value::String(if param.default.is_some() || param.variadic {
                    b"<optional>".to_vec().into()
                } else {
                    b"<required>".to_vec().into()
                }),
            );
        }
        properties.insert(
            ArrayKey::String(b"parameter".to_vec()),
            Value::Array(Rc::new(entries)),
        );
    }
    properties
}
