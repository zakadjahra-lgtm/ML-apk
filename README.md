# MC Launcher — لانشر ماين كرافت (Java) بلغة Rust لأندرويد

لانشر بنواة Rust وواجهة [Slint](https://slint.dev)، يعمل على أندرويد (وعلى الكمبيوتر للتجربة).

## ماذا يفعل الآن؟

- يجلب قائمة إصدارات ماين كرافت الرسمية من خوادم Mojang العامة (بدون تسجيل دخول).
- يحمّل ملف اللعبة (client jar) والمكتبات والـ assets مع التحقق من SHA-1.
- حساب **أوفلاين**: اسم لاعب + UUID ثابت بنفس خوارزمية اللعبة (`OfflinePlayer:<name>`).
  يصلح للعب الفردي وLAN والسيرفرات ذات `online-mode=false`. السيرفرات الأونلاين تحتاج حساب Microsoft.
- يبني أمر تشغيل Java الصحيح (JVM args + classpath + game args) ويشغّله.
- يحفظ الإعدادات في `launcher.json`.

## هيكل المشروع

| الملف | الوظيفة |
|---|---|
| `src/version.rs` | أنواع manifest وملف الإصدار وتقييم القواعد (rules) |
| `src/download.rs` | تحميل الملفات بالتوازي مع التحقق |
| `src/account.rs` | حساب أوفلاين وUUID |
| `src/launch.rs` | بناء وتشغيل أمر Java |
| `src/config.rs` | حفظ الإعدادات |
| `src/lib.rs` | ربط الواجهة بالنواة + `android_main` |
| `ui/main.slint` | الواجهة |

## البناء لأندرويد

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>

cargo apk build --release --lib --target aarch64-linux-android
# أو للتثبيت المباشر على هاتف موصول:
cargo apk run --release --lib --target aarch64-linux-android
```

## التجربة على الكمبيوتر

```bash
cargo run --bin mc_launcher_desktop
cargo test
```

## ⚠️ حدود مهمة (اقرأها)

1. **لم أستطع تجميع المشروع** في بيئتي (لا يوجد وصول لـ crates.io)، فالكود مكتوب بعناية لكنه غير مُختبَر.
   قد تظهر أخطاء بسيطة عند أول `cargo build` (إصدارات المكتبات مثلاً).
2. **تشغيل اللعبة فعلياً على أندرويد** يحتاج أكثر من هذا اللانشر:
   - أندرويد لا يحتوي JVM، فتحتاج تضمين JRE ARM64 (مثل ما يفعل PojavLauncher) وضبط `java_path` في `launcher.json`.
   - ماين كرافت تعتمد على LWJGL وOpenGL المكتبي، وأندرويد يوفّر OpenGL ES فقط، فتحتاج طبقة ترجمة (GL4ES / ANGLE / Zink) ومكتبات LWJGL مبنية لأندرويد.
   - الإصدارات القديمة تحتاج استخراج natives (غير منفّذ حالياً).
3. هذا المشروع لا يوزّع أي ملفات ماين كرافت؛ يحمّلها من خوادم Mojang الرسمية. اشترِ اللعبة لدعم المطورين.

## الخطوات التالية المقترحة

- تضمين JRE وطبقة GL داخل الـ APK وربط `java_path` تلقائياً.
- استخراج natives للإصدارات القديمة.
- تسجيل دخول Microsoft للسيرفرات الأونلاين.
- شاشة إعدادات (الرام، مسار Java) وإدارة عدة instances.
