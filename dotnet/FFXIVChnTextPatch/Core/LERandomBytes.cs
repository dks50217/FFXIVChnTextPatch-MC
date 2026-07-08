namespace FFXIVChnTextPatch.Core;

/// <summary>可讀寫的位元組緩衝，支援小端/大端與自動擴容。移植自 LERandomBytes.java（僅保留本專案用到的成員）。</summary>
public class LERandomBytes
{
    private int _point;
    private byte[] _work;
    private readonly bool _bigEndian;
    private readonly bool _increment;

    public LERandomBytes()
    {
        _work = Array.Empty<byte>();
        _increment = true;
    }

    public LERandomBytes(byte[] work)
    {
        _work = work;
    }

    public LERandomBytes(byte[] work, bool bigEndian, bool increment)
    {
        _work = work;
        _bigEndian = bigEndian;
        _increment = increment;
    }

    public byte[] GetWork() => _work;

    public int Length => _work.Length;

    public int Position => _point;

    public bool HasRemaining => _point < _work.Length;

    public void Seek(int pos) => _point = pos;

    public void ReadFully(byte[] bytes)
    {
        Array.Copy(_work, _point, bytes, 0, bytes.Length);
        _point += bytes.Length;
    }

    public byte ReadByte() => _work[_point++];

    private void Grow(int needed)
    {
        if (_increment && _work.Length - _point < needed)
        {
            var nwork = new byte[_point + needed];
            Array.Copy(_work, 0, nwork, 0, _point);
            _work = nwork;
        }
    }

    public void Write(byte[] bytes)
    {
        Grow(bytes.Length);
        Array.Copy(bytes, 0, _work, _point, bytes.Length);
        _point += bytes.Length;
    }

    public void WriteByte(byte b) => Write(new[] { b });

    public void WriteShort(int v) => WriteInt16(v);

    public void WriteInt16(int v)
    {
        var tmp = new byte[2];
        if (_bigEndian)
        {
            tmp[1] = (byte)v;
            tmp[0] = (byte)(v >> 8);
        }
        else
        {
            tmp[0] = (byte)v;
            tmp[1] = (byte)(v >> 8);
        }
        Write(tmp);
    }

    public void WriteInt(int v) => WriteInt32(v);

    public void WriteInt32(int v)
    {
        var tmp = new byte[4];
        if (_bigEndian)
        {
            tmp[3] = (byte)v;
            tmp[2] = (byte)(v >> 8);
            tmp[1] = (byte)(v >> 16);
            tmp[0] = (byte)(v >> 24);
        }
        else
        {
            tmp[0] = (byte)v;
            tmp[1] = (byte)(v >> 8);
            tmp[2] = (byte)(v >> 16);
            tmp[3] = (byte)(v >> 24);
        }
        Write(tmp);
    }
}
