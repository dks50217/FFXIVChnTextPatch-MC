using System.Text;

namespace FFXIVChnTextPatch.Core;

/// <summary>讀寫 conf/global.properties（Java Properties 格式相容：處理 \: \\ 等跳脫，UTF-8）。</summary>
public static class Config
{
    private static readonly Dictionary<string, string> Props = new();
    private static string _path = "";

    public static void Load(string path)
    {
        _path = path;
        Props.Clear();
        if (!File.Exists(path)) return;
        foreach (var raw in File.ReadLines(path, Encoding.UTF8))
        {
            var line = raw.TrimStart();
            if (line.Length == 0 || line[0] == '#' || line[0] == '!') continue;
            int sep = FindSeparator(line);
            if (sep < 0) continue;
            string key = Unescape(line[..sep].Trim());
            string value = Unescape(line[(sep + 1)..]);
            Props[key] = value;
        }
    }

    public static string? Get(string key) => Props.GetValueOrDefault(key);

    public static void Set(string key, string value) => Props[key] = value;

    public static void Save()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
        var sb = new StringBuilder();
        sb.AppendLine("#" + DateTime.Now.ToString("ddd MMM dd HH:mm:ss yyyy"));
        foreach (var (key, value) in Props)
            sb.AppendLine(Escape(key) + "=" + Escape(value));
        File.WriteAllText(_path, sb.ToString(), new UTF8Encoding(false));
    }

    private static int FindSeparator(string line)
    {
        for (int i = 0; i < line.Length; i++)
        {
            if (line[i] == '\\') { i++; continue; }
            if (line[i] == '=' || line[i] == ':') return i;
        }
        return -1;
    }

    private static string Unescape(string s)
    {
        var sb = new StringBuilder(s.Length);
        for (int i = 0; i < s.Length; i++)
        {
            char c = s[i];
            if (c == '\\' && i + 1 < s.Length)
            {
                char n = s[++i];
                switch (n)
                {
                    case 'n': sb.Append('\n'); break;
                    case 't': sb.Append('\t'); break;
                    case 'r': sb.Append('\r'); break;
                    case 'u' when i + 4 < s.Length:
                        sb.Append((char)Convert.ToInt32(s.Substring(i + 1, 4), 16));
                        i += 4;
                        break;
                    default: sb.Append(n); break;
                }
            }
            else sb.Append(c);
        }
        return sb.ToString();
    }

    private static string Escape(string s)
    {
        var sb = new StringBuilder(s.Length);
        foreach (char c in s)
        {
            if (c is '\\' or ':' or '=' or '#' or '!') sb.Append('\\');
            sb.Append(c);
        }
        return sb.ToString();
    }
}
