/// Common verb stems. We generate regex patterns that match common conjugations.
/// e.g. "walk" → walk|walks|walked|walking
pub const VERB_STEMS: &[&str] = &[
    // Movement
    "walk", "run", "jump", "climb", "crawl", "creep", "dance", "dash",
    "drift", "drop", "fall", "flee", "float", "fly", "follow", "gallop",
    "glide", "hurry", "jog", "leap", "limp", "march", "move", "pace",
    "plod", "plunge", "race", "retreat", "ride", "roll", "rush", "scramble",
    "shuffle", "skip", "slide", "slip", "sneak", "sprint", "stagger",
    "step", "stomp", "stride", "stroll", "stumble", "swagger", "swim",
    "swing", "tiptoe", "travel", "trek", "trip", "trudge", "wander",
    // Actions
    "break", "build", "burn", "carry", "catch", "chase", "chop", "clap",
    "clean", "close", "cook", "cover", "crash", "cross", "crush", "cut",
    "destroy", "dig", "drag", "draw", "dress", "drink", "drive", "drown",
    "eat", "enter", "escape", "examine", "explore", "fight", "fill",
    "find", "finish", "fix", "fold", "force", "gather", "give", "grab",
    "grasp", "grip", "hang", "hide", "hit", "hold", "hug", "hunt",
    "hurt", "kick", "kill", "kiss", "knock", "lay", "lead", "leave",
    "lick", "lift", "light", "lock", "make", "mark", "mix", "mount",
    "murder", "open", "pack", "paint", "pass", "pat", "pay", "pick",
    "place", "plant", "play", "point", "poke", "pour", "press", "pull",
    "punch", "push", "put", "raise", "reach", "read", "release",
    "remove", "repair", "replace", "rescue", "rest", "return", "rip",
    "rob", "rub", "save", "scratch", "seal", "search", "seize", "sell",
    "send", "set", "shake", "shape", "share", "shatter", "shove", "show",
    "shut", "sign", "sink", "sit", "slam", "slap", "slash", "sleep",
    "smoke", "snatch", "sort", "spend", "spill", "spit", "split",
    "spread", "squeeze", "stab", "stand", "start", "stay", "steal",
    "stick", "stop", "store", "stretch", "strike", "strip", "stroke",
    "study", "suck", "supply", "support", "surround", "survive", "sweep",
    "switch", "take", "taste", "teach", "tear", "test", "throw", "tie",
    "toss", "touch", "track", "trade", "train", "trap", "treat", "trim",
    "trust", "tuck", "tug", "twist", "type", "unfold", "unlock", "unpack",
    "untie", "use", "visit", "wake", "wash", "watch", "wave", "wear",
    "weave", "wipe", "work", "wrap", "write", "yank",
    // Communication
    "admit", "agree", "announce", "answer", "apologize", "argue", "ask",
    "beg", "blame", "call", "chat", "claim", "command", "comment",
    "complain", "confess", "confirm", "convince", "cry", "curse",
    "debate", "declare", "demand", "deny", "describe", "discuss",
    "dismiss", "exclaim", "explain", "greet", "growl", "grumble",
    "grunt", "hiss", "howl", "insist", "instruct", "interrupt",
    "introduce", "invite", "joke", "laugh", "lecture", "lie", "mention",
    "mock", "mumble", "murmur", "mutter", "nag", "narrate", "negotiate",
    "note", "object", "offer", "order", "persuade", "plead", "praise",
    "pray", "preach", "promise", "propose", "protest", "question",
    "quote", "rant", "reassure", "recommend", "refuse", "remind",
    "repeat", "reply", "report", "request", "respond", "reveal",
    "roar", "say", "scold", "scream", "shout", "shriek", "sigh", "sing",
    "snap", "sneer", "sob", "speak", "spell", "state", "stutter",
    "suggest", "swear", "talk", "tease", "tell", "thank", "threaten",
    "urge", "warn", "weep", "whisper", "wonder", "yell",
    // Perception / Mental
    "accept", "achieve", "allow", "approve", "assume", "attempt",
    "avoid", "believe", "bother", "care", "change", "choose", "compare",
    "consider", "contain", "continue", "control", "count", "create",
    "dare", "decide", "deliver", "depend", "deserve", "desire",
    "determine", "develop", "discover", "doubt", "dream", "earn",
    "encourage", "enjoy", "expect", "experience", "face", "fail", "fear",
    "feel", "forget", "forgive", "guess", "handle", "happen", "hate",
    "hear", "help", "hesitate", "hope", "ignore", "imagine", "impress",
    "improve", "include", "increase", "influence", "inform", "inspire",
    "intend", "interest", "involve", "join", "judge", "keep", "know",
    "lack", "learn", "like", "limit", "listen", "live", "lose", "love",
    "manage", "matter", "mean", "measure", "meet", "mind", "miss",
    "need", "notice", "obtain", "occur", "owe", "own", "perform",
    "permit", "plan", "please", "possess", "prefer", "prepare",
    "present", "prevent", "produce", "protect", "prove", "provide",
    "realize", "recall", "receive", "recognize", "reflect", "refuse",
    "regard", "regret", "relate", "remain", "remember", "require",
    "respect", "reveal", "risk", "satisfy", "seem", "sense", "serve",
    "settle", "solve", "sound", "suffer", "suppose", "suspect",
    "tend", "think", "understand", "value", "wait", "want", "warn",
    "win", "wish", "wonder", "worry",
    // Body / Physical states
    "ache", "bite", "bleed", "blink", "blush", "breathe", "chew",
    "choke", "cough", "cry", "faint", "flinch", "freeze", "frown",
    "gasp", "gaze", "glance", "glare", "grin", "groan", "heal",
    "itch", "kneel", "lean", "lunge", "nod", "pale", "pant", "peer",
    "perspire", "quiver", "shiver", "shrug", "smile", "snore", "squint",
    "stare", "starve", "strain", "swallow", "sweat", "swell", "tense",
    "tremble", "twitch", "wince", "wink", "yawn",
];

/// Common adjectives for fiction writing
pub const ADJECTIVES: &[&str] = &[
    // Size / Shape
    "big", "small", "tiny", "huge", "enormous", "massive", "vast", "little",
    "tall", "short", "long", "wide", "narrow", "thick", "thin", "slim",
    "broad", "round", "flat", "sharp", "steep", "deep", "shallow",
    // Appearance
    "beautiful", "handsome", "pretty", "ugly", "attractive", "gorgeous",
    "stunning", "elegant", "plain", "pale", "dark", "bright", "dull",
    "clean", "dirty", "neat", "messy", "rough", "smooth", "shiny",
    "worn", "ragged", "faded", "fresh", "crisp", "wrinkled",
    // Color
    "red", "blue", "green", "yellow", "black", "white", "grey", "gray",
    "brown", "golden", "silver", "crimson", "scarlet", "azure", "ivory",
    "amber", "violet", "pink", "orange", "bronze", "copper",
    // Texture / Material
    "soft", "hard", "wet", "dry", "cold", "hot", "warm", "cool",
    "frozen", "icy", "damp", "sticky", "slippery", "silky", "velvet",
    "wooden", "stone", "metal", "glass", "leather", "cotton",
    // Emotion / Character
    "happy", "sad", "angry", "afraid", "scared", "frightened", "terrified",
    "anxious", "nervous", "worried", "calm", "peaceful", "quiet", "loud",
    "gentle", "fierce", "brave", "cowardly", "bold", "shy", "proud",
    "humble", "kind", "cruel", "mean", "generous", "selfish", "honest",
    "clever", "stupid", "wise", "foolish", "curious", "suspicious",
    "jealous", "envious", "grateful", "bitter", "lonely", "desperate",
    "hopeful", "hopeless", "guilty", "innocent", "stubborn", "patient",
    "impatient", "cheerful", "gloomy", "grumpy", "pleasant", "rude",
    "polite", "arrogant", "modest", "confident", "insecure",
    // State / Condition
    "alive", "dead", "awake", "asleep", "conscious", "unconscious",
    "healthy", "sick", "ill", "injured", "wounded", "tired", "exhausted",
    "weak", "strong", "powerful", "helpless", "hungry", "thirsty",
    "drunk", "sober", "pregnant", "blind", "deaf", "mute", "lame",
    // Quality
    "good", "bad", "great", "terrible", "wonderful", "awful", "excellent",
    "perfect", "poor", "fine", "nice", "lovely", "horrible", "dreadful",
    "magnificent", "superb", "decent", "fair", "wicked", "evil",
    // Time / Age
    "old", "young", "ancient", "modern", "new", "recent", "early", "late",
    "quick", "slow", "fast", "sudden", "gradual", "brief", "eternal",
    // General
    "strange", "odd", "weird", "normal", "ordinary", "unusual",
    "familiar", "foreign", "rare", "common", "special", "important",
    "dangerous", "safe", "secret", "hidden", "obvious", "clear",
    "certain", "uncertain", "possible", "impossible", "necessary",
    "empty", "full", "open", "closed", "free", "busy", "ready",
    "heavy", "light", "loose", "tight", "rich", "poor", "expensive",
    "cheap", "real", "fake", "true", "false", "wild", "tame",
    "raw", "ripe", "rotten", "whole", "broken", "complete", "missing",
    "different", "similar", "same", "opposite", "separate", "single",
    "double", "final", "main", "major", "minor",
];

/// Common adverbs for fiction writing
pub const ADVERBS: &[&str] = &[
    // Manner
    "quickly", "slowly", "carefully", "carelessly", "quietly", "loudly",
    "softly", "gently", "roughly", "firmly", "lightly", "heavily",
    "eagerly", "reluctantly", "deliberately", "accidentally",
    "gracefully", "awkwardly", "calmly", "angrily", "sadly", "happily",
    "nervously", "anxiously", "confidently", "shyly", "boldly",
    "bravely", "fiercely", "desperately", "frantically", "furiously",
    "silently", "wearily", "lazily", "briskly", "casually",
    "cautiously", "cheerfully", "coldly", "coolly", "curiously",
    "darkly", "deeply", "dryly", "easily", "faintly", "faithfully",
    "flatly", "fondly", "foolishly", "freely", "grimly", "greedily",
    "hastily", "helplessly", "honestly", "hungrily", "innocently",
    "kindly", "knowingly", "lovingly", "madly", "miserably",
    "mysteriously", "neatly", "obediently", "painfully", "patiently",
    "playfully", "politely", "poorly", "properly", "proudly",
    "recklessly", "rudely", "ruthlessly", "sharply", "smoothly",
    "stiffly", "stubbornly", "suspiciously", "sweetly", "tightly",
    "tiredly", "viciously", "warmly", "weakly", "wickedly", "wildly",
    "wisely",
    // Manner (extended)
    "abnormally", "absentmindedly", "adventurously", "arrogantly",
    "bashfully", "beautifully", "bitterly", "bleakly", "blindly",
    "blissfully", "boastfully", "briefly", "brightly", "broadly",
    "busily", "cleverly", "closely", "coaxingly", "colorfully",
    "courageously", "crossly", "cruelly", "daintily", "deceivingly",
    "defiantly", "delightfully", "diligently", "dimly", "doubtfully",
    "dreamily", "elegantly", "energetically", "enthusiastically",
    "evenly", "excitedly", "ferociously", "fervently", "fortunately",
    "frankly", "frenetically", "frightfully", "generously", "gladly",
    "gleefully", "gratefully", "healthily", "helpfully", "hopelessly",
    "inquisitively", "intently", "interestingly", "inwardly",
    "irritably", "jaggedly", "jealously", "joshingly", "joyfully",
    "joyously", "jovially", "jubilantly", "judgementally", "justly",
    "keenly", "kiddingly", "kindheartedly", "kissingly", "knavishly",
    "knottily", "knowledgeably", "kookily", "limply", "loftily",
    "longingly", "loosely", "loyally", "majestically", "meaningfully",
    "mechanically", "merrily", "mockingly", "mortally", "needily",
    "nicely", "noisily", "obnoxiously", "oddly", "offensively",
    "officially", "openly", "optimistically", "overconfidently",
    "owlishly", "partially", "physically", "positively", "potentially",
    "powerfully", "punctually", "quaintly", "quarrelsomely",
    "queasily", "queerly", "questionably", "questioningly", "quirkily",
    "quizzically", "rapidly", "readily", "reassuringly", "regularly",
    "reproachfully", "restfully", "righteously", "rightfully",
    "rigidly", "safely", "scarily", "searchingly", "sedately",
    "seemingly", "selfishly", "separately", "seriously", "shakily",
    "sheepishly", "shrilly", "sleepily", "smoothly", "solemnly",
    "solidly", "speedily", "stealthily", "sternly", "strictly",
    "successfully", "surprisingly", "swiftly", "sympathetically",
    "tenderly", "tensely", "thankfully", "thoughtfully",
    "triumphantly", "truthfully", "unabashedly", "unaccountably",
    "unbearably", "unethically", "unexpectedly", "unfortunately",
    "unimpressively", "unnaturally", "unnecessarily", "upwardly",
    "urgently", "usefully", "uselessly", "vacantly", "vaguely",
    "vainly", "valiantly", "verbally", "victoriously", "violently",
    "vivaciously", "voluntarily", "wetly", "wholly", "willfully",
    "woefully", "wonderfully", "worriedly", "wrongly", "yawningly",
    "yearningly", "yieldingly", "youthfully", "zealously",
    "zestfully", "zestily",
    // Degree
    "absolutely", "almost", "barely", "completely", "considerably",
    "entirely", "especially", "exactly", "extremely", "fairly",
    "fully", "greatly", "hardly", "highly", "incredibly", "intensely",
    "merely", "mostly", "nearly", "partly", "perfectly", "practically",
    "purely", "quite", "rather", "remarkably", "scarcely", "slightly",
    "somewhat", "strongly", "terribly", "thoroughly", "totally",
    "tremendously", "truly", "utterly", "vastly", "virtually",
    "enormously",
    // Time / Frequency
    "afterwards", "already", "immediately", "instantly", "eventually",
    "finally", "formerly", "frequently", "lately", "meanwhile",
    "occasionally", "once", "presently", "previously", "promptly",
    "recently", "repeatedly", "shortly", "simultaneously", "soon",
    "subsequently", "suddenly", "temporarily",
    "always", "annually", "continually", "daily", "hourly", "monthly",
    "never", "often", "rarely", "seldom", "sometimes", "yearly",
    // Certainty / Emphasis
    "actually", "apparently", "certainly", "clearly", "definitely",
    "evidently", "inevitably", "naturally", "obviously", "presumably",
    "probably", "supposedly", "surely", "undoubtedly",
    "commonly", "correctly", "equally", "generally",
];

/// Build a regex pattern that matches common conjugations of a verb stem.
/// "walk" → (?:walk|walks|walked|walking)
fn verb_stem_to_pattern(stem: &str) -> String {
    let chars: Vec<char> = stem.chars().collect();
    let last = chars.last().copied().unwrap_or(' ');
    let len = chars.len();

    let mut forms = vec![stem.to_string()];

    // Handle irregular common verbs
    match stem {
        "run" => { forms.extend(["runs", "ran", "running"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "eat" => { forms.extend(["eats", "ate", "eating", "eaten"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "drink" => { forms.extend(["drinks", "drank", "drinking", "drunk"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "give" => { forms.extend(["gives", "gave", "giving", "given"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "take" => { forms.extend(["takes", "took", "taking", "taken"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "break" => { forms.extend(["breaks", "broke", "breaking", "broken"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "speak" => { forms.extend(["speaks", "spoke", "speaking", "spoken"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "write" => { forms.extend(["writes", "wrote", "writing", "written"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "drive" => { forms.extend(["drives", "drove", "driving", "driven"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "ride" => { forms.extend(["rides", "rode", "riding", "ridden"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "hide" => { forms.extend(["hides", "hid", "hiding", "hidden"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "fall" => { forms.extend(["falls", "fell", "falling", "fallen"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "know" => { forms.extend(["knows", "knew", "knowing", "known"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "grow" => { forms.extend(["grows", "grew", "growing", "grown"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "throw" => { forms.extend(["throws", "threw", "throwing", "thrown"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "fly" => { forms.extend(["flies", "flew", "flying", "flown"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "swim" => { forms.extend(["swims", "swam", "swimming", "swum"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "sing" => { forms.extend(["sings", "sang", "singing", "sung"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "sit" => { forms.extend(["sits", "sat", "sitting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "stand" => { forms.extend(["stands", "stood", "standing"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "say" => { forms.extend(["says", "said", "saying"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "tell" => { forms.extend(["tells", "told", "telling"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "feel" => { forms.extend(["feels", "felt", "feeling"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "think" => { forms.extend(["thinks", "thought", "thinking"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "see" => { forms.extend(["sees", "saw", "seeing", "seen"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "hear" => { forms.extend(["hears", "heard", "hearing"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "find" => { forms.extend(["finds", "found", "finding"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "lose" => { forms.extend(["loses", "lost", "losing"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "leave" => { forms.extend(["leaves", "left", "leaving"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "hold" => { forms.extend(["holds", "held", "holding"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "make" => { forms.extend(["makes", "made", "making"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "come" => { forms.extend(["comes", "came", "coming"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "keep" => { forms.extend(["keeps", "kept", "keeping"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "begin" => { forms.extend(["begins", "began", "beginning", "begun"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "fight" => { forms.extend(["fights", "fought", "fighting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "catch" => { forms.extend(["catches", "caught", "catching"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "teach" => { forms.extend(["teaches", "taught", "teaching"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "pay" => { forms.extend(["pays", "paid", "paying"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "meet" => { forms.extend(["meets", "met", "meeting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "win" => { forms.extend(["wins", "won", "winning"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "hit" => { forms.extend(["hits", "hitting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "cut" => { forms.extend(["cuts", "cutting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "put" => { forms.extend(["puts", "putting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "set" => { forms.extend(["sets", "setting"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "lie" => { forms.extend(["lies", "lay", "lain", "lying", "lied"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        "die" => { forms.extend(["dies", "died", "dying"].iter().map(|s| s.to_string())); return format!("(?:{})", forms.join("|")); }
        _ => {}
    }

    // Regular conjugation rules
    if last == 'e' {
        // dance → dances, danced, dancing
        let base: String = chars[..len-1].iter().collect();
        forms.push(format!("{}es", stem));    // dances
        forms.push(format!("{}ed", base));    // danced (drop e, add ed)
        forms.push(format!("{}ing", base));   // dancing (drop e, add ing)
    } else if last == 'y' && len > 1 && !"aeiou".contains(chars[len-2]) {
        // carry → carries, carried, carrying
        let base: String = chars[..len-1].iter().collect();
        forms.push(format!("{}ies", base));   // carries
        forms.push(format!("{}ied", base));   // carried
        forms.push(format!("{}ying", stem));  // carrying
    } else if "bdfgklmnprt".contains(last) && len > 2
        && "aeiou".contains(chars[len-2])
        && !("aeiou".contains(chars[len-3])) {
        // stop → stops, stopped, stopping (double consonant)
        forms.push(format!("{}s", stem));
        forms.push(format!("{}{}ed", stem, last));
        forms.push(format!("{}{}ing", stem, last));
    } else {
        // walk → walks, walked, walking
        forms.push(format!("{}s", stem));
        forms.push(format!("{}ed", stem));
        forms.push(format!("{}ing", stem));
    }

    format!("(?:{})", forms.join("|"))
}

/// Expand POS tags in a query string.
/// {verb} → regex matching any common verb
/// {adjective} or {adj} → any common adjective
/// {adverb} or {adv} → any common adverb
pub fn expand_pos_tags(query: &str) -> String {
    let mut result = query.to_string();

    if result.contains("{verb}") {
        let verb_pattern = VERB_STEMS.iter()
            .map(|s| verb_stem_to_pattern(s))
            .collect::<Vec<_>>()
            .join("|");
        result = result.replace("{verb}", &format!("(?:{})", verb_pattern));
    }

    if result.contains("{adjective}") || result.contains("{adj}") {
        let adj_pattern = ADJECTIVES.join("|");
        let replacement = format!("(?:{})", adj_pattern);
        result = result.replace("{adjective}", &replacement);
        result = result.replace("{adj}", &replacement);
    }

    if result.contains("{adverb}") || result.contains("{adv}") {
        let adv_pattern = ADVERBS.join("|");
        let replacement = format!("(?:{})", adv_pattern);
        result = result.replace("{adverb}", &replacement);
        result = result.replace("{adv}", &replacement);
    }

    result
}
