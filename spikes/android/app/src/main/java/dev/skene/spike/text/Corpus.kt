package dev.skene.spike.text

/**
 * The spike's texts: one string per leaf, in the order the Rust tree creates its leaves.
 *
 * Each card holds ten leaves: a paragraph (0), three weighted items (1, 2, 3), a fixed-width
 * label (4) beside a weighted item (5), and four wrapping tags (6..9).
 */
class Corpus(val cards: Int) {
  val leaves = cards * LEAVES_PER_CARD
  private val texts = Array(leaves) { make(it) }

  fun text(id: Int): String = texts[id]

  /** The style index of a leaf: 0 paragraph, 1 item, 2 tag. */
  fun style(id: Int): Int = when (id % LEAVES_PER_CARD) {
    0 -> 0
    in 6..9 -> 2
    else -> 1
  }

  /** The script family a leaf's text leads with, for per-script reporting. */
  fun script(id: Int): String = SCRIPTS[(id / LEAVES_PER_CARD + id) % SCRIPTS.size]

  private fun make(id: Int): String {
    val slot = id % LEAVES_PER_CARD
    val card = id / LEAVES_PER_CARD
    val lead = POOLS.getValue(script(id))

    return when (slot) {
      0 -> {
        val second = POOLS.getValue(SCRIPTS[(card + 2) % SCRIPTS.size])
        val third = POOLS.getValue(SCRIPTS[(card + 4) % SCRIPTS.size])
        "${lead[card % lead.size]} ${second[(card + 1) % second.size]} ${third[(card + 2) % third.size]} $card"
      }
      in 6..9 -> TAGS[(card * 3 + slot) % TAGS.size]
      4 -> LABELS[(card + slot) % LABELS.size]
      else -> "${lead[(card + slot) % lead.size]} $id"
    }
  }

  companion object {
    const val LEAVES_PER_CARD = 10

    /** Text sizes in pixels per style index. */
    val SIZES = floatArrayOf(44f, 40f, 34f)

    val SCRIPTS = arrayOf("latin", "cjk", "emoji", "thai", "arabic", "mixed")

    val POOLS = mapOf(
      "latin" to arrayOf(
        "The quick brown fox jumps over the lazy dog",
        "Layout is measured by the platform's own engine, inside our own views",
        "Internationalization and localization are long words to wrap",
        "Supercalifragilisticexpialidocious breaks where the line fills",
        "Résumé, naïve café: diacritics and ligatures ffi fl",
      ),
      "cjk" to arrayOf(
        "汉字的排版需要正确的换行规则和标点挤压",
        "日本語のテキストは句読点の後で改行されます。",
        "한국어 텍스트 줄 바꿈 테스트입니다",
        "门骨直这三个字在中文和日文中的字形不同",
        "東京都渋谷区の天気は晴れ、気温は二十度です",
      ),
      "emoji" to arrayOf(
        "Family 👨‍👩‍👧‍👦 and flags 🇫🇮🇯🇵🇧🇷",
        "Thumbs 👍🏽 heart ❤️ party 🎉 rocket 🚀",
        "🧑🏿‍💻 coding, 🏳️‍🌈 pride, 🐻‍❄️ polar bear",
        "Keycaps 1️⃣2️⃣3️⃣ and skin tones 👋🏻👋🏿",
      ),
      "thai" to arrayOf(
        "ภาษาไทยไม่มีการเว้นวรรคระหว่างคำในประโยค",
        "การตัดคำภาษาไทยต้องใช้พจนานุกรมเพื่อหาขอบเขตของคำ",
        "สวัสดีครับ ยินดีต้อนรับสู่การทดสอบการจัดวางข้อความ",
      ),
      "arabic" to arrayOf(
        "النص العربي يكتب من اليمين إلى اليسار",
        "مرحبا بالعالم، هذا اختبار لتخطيط النصوص",
        "الحروف العربية تتصل ببعضها في الكلمة الواحدة",
      ),
      "mixed" to arrayOf(
        "Mixed English العربية 中文 ไทย 😀 in one run",
        "Price: ١٢٣ درهم or 456 円 — ok 👍",
        "שלום world, مرحبا 世界, สวัสดี",
      ),
    )

    val LABELS = arrayOf("Name", "名前", "الاسم", "ชื่อ", "Status 🚦", "Überschrift")

    val TAGS = arrayOf(
      "rust", "kotlin", "日本", "中文", "한국", "ไทย", "عربي", "🎉", "👨‍👩‍👧", "layout",
      "taffy", "minikin", "jni", "compose", "text", "cache",
    )
  }
}
