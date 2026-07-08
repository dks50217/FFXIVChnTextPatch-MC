namespace FFXIVChnTextPatch.Core;

/// <summary>把資料切成最大 16000 bytes 的 deflate 區塊，組成 SqPack type 2 檔案。移植自 BinaryBlockBuilder.java。</summary>
public class BinaryBlockBuilder
{
    private readonly byte[] _data;
    private int _dataOffset;

    public BinaryBlockBuilder(byte[] data) => _data = data;

    public byte[] BuildBlock()
    {
        var dataBodys = new List<LERandomBytes>();
        var leFile = new LERandomBytes(_data);
        int uncompressedSize = leFile.Length;
        int partCount = (int)MathF.Ceiling(uncompressedSize / 16000.0f);
        int dataHeaderLength = 24 + partCount * 8;
        if (dataHeaderLength < 128)
            dataHeaderLength = 128;
        else
            dataHeaderLength += 128 - dataHeaderLength % 128;

        var dataHeader = new LERandomBytes(new byte[dataHeaderLength]);
        dataHeader.WriteInt(dataHeaderLength);
        dataHeader.WriteInt(2);
        dataHeader.WriteInt(uncompressedSize);
        dataHeader.WriteInt(0);
        dataHeader.WriteInt(0);
        dataHeader.WriteInt(partCount);
        for (int i = 1; i <= partCount; i++)
        {
            int partSize = i == partCount ? leFile.Length - leFile.Position : 16000;
            var tmpPart = new byte[partSize];
            leFile.ReadFully(tmpPart);
            var compr = SqPackZlib.Compress(tmpPart);
            int compressedSize = compr.Length;
            int paddingSize = 128 - (compressedSize + 16) % 128;
            var dataBody = new LERandomBytes(new byte[compressedSize + 16 + paddingSize]);
            dataBody.WriteInt(16);
            dataBody.WriteInt(0);
            dataBody.WriteInt(compressedSize);
            dataBody.WriteInt(partSize);
            dataBody.Write(compr);
            dataBodys.Add(dataBody);
            dataHeader.WriteInt(_dataOffset);
            dataHeader.WriteShort(compressedSize + 16 + paddingSize);
            dataHeader.WriteShort(partSize);
            _dataOffset += compressedSize + 16 + paddingSize;
        }
        dataHeader.Seek(12);
        dataHeader.WriteInt(_dataOffset / 128);
        dataHeader.Seek(16);
        dataHeader.WriteInt(_dataOffset / 128);

        var block = new MemoryStream();
        block.Write(dataHeader.GetWork());
        foreach (var body in dataBodys)
            block.Write(body.GetWork());
        return block.ToArray();
    }
}

/// <summary>從修改後的 row map 重建 EXD 二進位（big-endian）。移植自 EXDFBuilder.java。</summary>
public class EXDFBuilder
{
    private readonly Dictionary<int, byte[]> _entries;

    public EXDFBuilder(Dictionary<int, byte[]> entries) => _entries = entries;

    public byte[] BuildExdf()
    {
        int headerSize = 0, bodySize = 0;
        foreach (var entry in _entries)
        {
            headerSize += 8;
            bodySize += entry.Value.Length + 6;
        }
        int dataOffset = 32 + headerSize;

        var header = new LERandomBytes(new byte[32], bigEndian: true, increment: false);
        var dataHeader = new LERandomBytes(new byte[headerSize], bigEndian: true, increment: false);
        var dataBodys = new LERandomBytes(new byte[bodySize], bigEndian: true, increment: false);

        foreach (var (index, data) in _entries.OrderBy(kv => kv.Key))
        {
            dataHeader.WriteInt(index);
            dataHeader.WriteInt(dataOffset);
            dataBodys.WriteInt(data.Length);
            dataBodys.WriteShort(1);
            dataBodys.Write(data);
            dataOffset += data.Length + 4 + 2;
        }
        header.Write(new byte[] { 69, 88, 68, 70, 0, 2, 0, 0 }); // "EXDF", version 2
        header.WriteInt(dataHeader.Length);
        header.WriteInt(dataBodys.Length);

        var result = new MemoryStream();
        result.Write(header.GetWork());
        result.Write(dataHeader.GetWork());
        result.Write(dataBodys.GetWork());
        return result.ToArray();
    }
}

/// <summary>重建 TEX 字型的 SqPack type 4 檔案。移植自 TexBlockBuilder.java（含其單一 mip 假設）。</summary>
public class TexBlockBuilder
{
    private readonly byte[] _data;

    public TexBlockBuilder(byte[] data) => _data = data;

    public byte[] BuildBlock()
    {
        int dataOffset = 0;
        var dataBodys = new List<LERandomBytes>();
        var leData = new LERandomBytes(_data);
        leData.Seek(28);
        var mipOffsetBytes = new byte[4];
        leData.ReadFully(mipOffsetBytes);
        int mipOffsetIndex = BitConverter.ToInt32(mipOffsetBytes);
        leData.Seek(0);
        var texHeader = new byte[mipOffsetIndex];
        leData.ReadFully(texHeader);

        int textureType = BitConverter.ToInt16(texHeader, 4);
        int width = BitConverter.ToInt16(texHeader, 8);
        int height = BitConverter.ToInt16(texHeader, 10);
        int mipCount = BitConverter.ToInt16(texHeader, 14);

        int uncompressedSize = leData.Length;
        int partCount = (int)MathF.Ceiling(uncompressedSize / 16000.0f);
        int dataHeaderLength = 24 + mipCount * 20 + partCount * 2;
        if (dataHeaderLength < 128)
            dataHeaderLength = 128;
        else
            dataHeaderLength += 128 - dataHeaderLength % 128;

        int uncompMipSize = width * height;
        if (textureType == 5184)
            uncompMipSize = width * height * 2;

        var dataHeader = new LERandomBytes(new byte[dataHeaderLength]);
        dataHeader.WriteInt(dataHeaderLength);
        dataHeader.WriteInt(4);
        dataHeader.WriteInt(uncompressedSize);
        dataHeader.WriteInt(0);
        dataHeader.WriteInt(0);
        dataHeader.WriteInt(mipCount);
        for (int o = 0; o < mipCount; o++)
        {
            dataHeader.WriteInt(mipOffsetIndex);
            int lengthPos = dataHeader.Position;
            dataHeader.WriteInt(0);
            dataHeader.WriteInt(uncompMipSize);
            dataHeader.WriteInt(0);
            dataHeader.WriteInt(partCount);
            for (int k = 1; k <= partCount; k++)
            {
                int partSize = k == partCount ? leData.Length - leData.Position : 16000;
                var tmpPart = new byte[partSize];
                leData.ReadFully(tmpPart);
                var compr = SqPackZlib.Compress(tmpPart);
                int compressedSize = compr.Length;
                int paddingSize = 128 - (compressedSize + 16) % 128;
                var dataBody = new LERandomBytes(new byte[compressedSize + 16 + paddingSize]);
                dataBody.WriteInt(16);
                dataBody.WriteInt(0);
                dataBody.WriteInt(compressedSize);
                dataBody.WriteInt(partSize);
                dataBody.Write(compr);
                dataBodys.Add(dataBody);
                dataHeader.WriteShort(compressedSize + 16 + paddingSize);
                dataOffset += compressedSize + 16 + paddingSize;
            }
            dataHeader.Seek(12);
            dataHeader.WriteInt(dataOffset / 128);
            dataHeader.Seek(16);
            dataHeader.WriteInt(dataOffset / 128);
            dataHeader.Seek(lengthPos);
            dataHeader.WriteInt(dataOffset + texHeader.Length);
            dataOffset = 0;
        }

        var block = new MemoryStream();
        block.Write(dataHeader.GetWork());
        block.Write(texHeader);
        foreach (var body in dataBodys)
            block.Write(body.GetWork());
        int finalPadding = 128 - (int)(block.Length % 128);
        block.Write(new byte[finalPadding]);
        return block.ToArray();
    }
}
