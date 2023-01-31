# 使用 Linux 版本
一部分用戶在 SteamOS 上進行遊戲，可以使用下列的步驟在 SteamOS 桌面模式上運行並打漢化補丁。

## 準備
### Libs
專案本身 build.gradle 中不包含， `com.univocity.parsers.csv` 這個 package 需要 [下載](https://oss.sonatype.org/service/local/repositories/releases/content/com/univocity/univocity-parsers/2.9.1/univocity-parsers-2.9.1.jar) ，並放到 `專案目錄/libs` 中。  
### 安裝 JDK 11  
```shell
yay -S jdk11-openjdk
```
如果已经有多个 Java 版本，使用下列指令切换到 JDK11
```shell
sudo archlinux-java set java-11-openjdk
```
### 安裝 Gradle 6
[下载](https://downloads.gradle-dn.com/distributions/gradle-6.8.3-bin.zip)  Gradle，解壓並配置環境變量
```shell
export PATH="/path/to/gradle-6.8.3/bin:$PATH"
```

## Run

每次運行前都使用下列命令配置環境變量
```shell
export PATH="/path/to/gradle-6.8.3/bin:$PATH"
```

在 Terminal 中，執行

```shell
gradle run
```