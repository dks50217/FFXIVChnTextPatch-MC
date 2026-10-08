namespace FFXIVChnTextPatch.Core;

public static class HexUtils
{
    public static byte[] HexStringToBytes(string hex) =>
        Convert.FromHexString(hex.Replace(" ", ""));
}
