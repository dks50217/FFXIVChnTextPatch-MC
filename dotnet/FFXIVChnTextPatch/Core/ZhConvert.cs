using System.Text;

namespace FFXIVChnTextPatch.Core;

/// <summary>
/// 簡→繁（台灣正體＋台灣用語）轉換，等同 OpenCC 的 s2twp 再加 FFXIV 專屬校訂。
/// 演算法: 最長正向匹配，三輪——
///   1. GPPhrases（GP 版的 FFXIV 例外詞彙表）+ STPhrases + STCharacters（簡→繁，詞優先於單字）
///   2. TWPhrases（台灣用語）
///   3. TWVariants（台灣異體字）
/// UserPhrases.txt 是使用者自訂表，同時掛在第 1、2 輪的最前面（簡體或繁體原詞都吃），優先權最高。
/// 字典都是 TSV，放在 resource/opencc/。
/// </summary>
public static class ZhConvert
{
    private static readonly Lazy<(Dictionary<string, string> Dict, int MaxLen, HashSet<char> FirstChars)[]> Rounds = new(() =>
    [
        Load("UserPhrases.txt", "GPPhrases.txt", "STPhrases.txt", "STCharacters.txt"),
        Load("UserPhrases.txt", "TWPhrases.txt"),
        Load("TWVariants.txt"),
    ]);

    private static (Dictionary<string, string>, int, HashSet<char>) Load(params string[] files)
    {
        var dict = new Dictionary<string, string>();
        int maxLen = 1;
        foreach (var file in files)
        {
            string path = AppEnv.P("resource", "opencc", file);
            if (!File.Exists(path)) continue; // UserPhrases.txt 可有可無
            foreach (var line in File.ReadLines(path))
            {
                if (line.Length == 0 || line[0] == '#') continue;
                int tab = line.IndexOf('\t');
                if (tab <= 0) continue;
                string key = line[..tab];
                if (dict.ContainsKey(key)) continue;  // 先載入的字典優先
                int sp = line.IndexOf(' ', tab + 1);  // 一對多取第一個候選（OpenCC 預設）
                dict[key] = sp < 0 ? line[(tab + 1)..] : line[(tab + 1)..sp];
                maxLen = Math.Max(maxLen, key.Length);
            }
        }
        // GPPhrases 有引號/英文名開頭的 key，不能用「非 CJK 就跳過」的快速通道，改記 key 首字元集合
        var firstChars = new HashSet<char>(dict.Keys.Select(k => k[0]));
        return (dict, maxLen, firstChars);
    }

    public static string S2Tw(string text)
    {
        foreach (var (dict, maxLen, firstChars) in Rounds.Value)
            text = Apply(text, dict, maxLen, firstChars);
        return text;
    }

    private static string Apply(string s, Dictionary<string, string> dict, int maxLen, HashSet<char> firstChars)
    {
        var lookup = dict.GetAlternateLookup<ReadOnlySpan<char>>();
        var sb = new StringBuilder(s.Length);
        int i = 0;
        while (i < s.Length)
        {
            if (!firstChars.Contains(s[i])) { sb.Append(s[i++]); continue; }  // 非任何 key 首字元，直接放行
            bool matched = false;
            for (int len = Math.Min(maxLen, s.Length - i); len >= 1 && !matched; len--)
                if (lookup.TryGetValue(s.AsSpan(i, len), out var to))
                {
                    sb.Append(to);
                    i += len;
                    matched = true;
                }
            if (!matched) sb.Append(s[i++]);
        }
        return sb.ToString();
    }
}
