use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;

pub struct PaletteState {
    pub db: Mutex<Connection>,
}

// ---- Structs ----

#[derive(Serialize, Clone)]
pub struct Palette {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub group_count: i64,
    pub entry_count: i64,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct WordGroup {
    pub id: i64,
    pub palette_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub entry_count: i64,
}

#[derive(Serialize, Clone)]
pub struct WordSection {
    pub id: i64,
    pub group_id: i64,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Serialize, Clone)]
pub struct WordEntry {
    pub id: i64,
    pub group_id: i64,
    pub section_id: Option<i64>,
    pub word: String,
    pub sort_order: i64,
}

#[derive(Serialize)]
pub struct WordGroupDetail {
    pub group: WordGroup,
    pub sections: Vec<WordSection>,
    pub entries: Vec<WordEntry>,
}

#[derive(Serialize)]
pub struct PaletteDetail {
    pub palette: Palette,
    pub groups: Vec<WordGroup>,
}

// ---- Init & Schema ----

pub fn init_palette_db(path: &str) -> rusqlite::Result<Connection> {
    let is_new = !std::path::Path::new(path).exists();
    let conn = Connection::open(path)?;

    conn.execute_batch("
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS palettes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            is_system INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS word_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            palette_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (palette_id) REFERENCES palettes(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS word_sections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (group_id) REFERENCES word_groups(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS word_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            group_id INTEGER NOT NULL,
            section_id INTEGER,
            word TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (group_id) REFERENCES word_groups(id) ON DELETE CASCADE,
            FOREIGN KEY (section_id) REFERENCES word_sections(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_word_groups_palette ON word_groups(palette_id);
        CREATE INDEX IF NOT EXISTS idx_word_entries_group ON word_entries(group_id);
        CREATE INDEX IF NOT EXISTS idx_word_entries_section ON word_entries(section_id);
        CREATE INDEX IF NOT EXISTS idx_word_sections_group ON word_sections(group_id);
    ")?;

    if is_new {
        seed_starter_data(&conn)?;
    }

    Ok(conn)
}

// ---- Seed Data ----

fn seed_starter_data(conn: &Connection) -> rusqlite::Result<()> {
    // Create the system default palette
    conn.execute(
        "INSERT INTO palettes (name, description, is_system, is_active, sort_order) VALUES (?1, ?2, 1, 1, 0)",
        params!["The Writer's Palette", "A comprehensive reference for emotion beats, sensory details, movement, setting, and dialogue"],
    )?;
    let palette_id = conn.last_insert_rowid();

    // Emotion groups: each has 4 standard sections
    let groups: Vec<(&str, &str, Vec<(&str, Vec<&str>)>)> = vec![
        // ── Anger Family ──
        ("Anger", "The hot, energizing feeling that something is wrong and someone is to blame", vec![
            ("Physical Signals", vec![
                "hands curling into fists at the sides",
                "jaw clenched so tight the muscles visibly twitch",
                "nostrils flaring on each exhale",
                "eyes narrowing to slits, unblinking and locked on target",
                "lips pressed into a bloodless line",
                "chin thrust forward, face pulled taut and compact",
                "stabbing the air with a pointed finger",
                "invading someone's space — chest to chest, close enough to feel breath",
                "pacing like a caged animal, unable to hold still",
                "slamming a fist on the table hard enough to rattle the glasses",
                "speaking through clenched teeth, each word bitten off",
                "voice dropping to a dangerous, controlled quiet",
                "throwing an object — a book, a glass — watching it shatter",
                "shoving a chair back so hard it topples",
                "a vein pulsing visibly at the temple",
                "shoulders hiked up toward the ears, rigid as stone",
                "baring teeth in something that is not a smile",
                "gripping the edge of a counter until the knuckles go white",
                "storming out of a room, door slamming behind hard enough to shake the frame",
                "sudden, jerky movements — hands thrown up, head whipped around",
            ]),
            ("Internal Sensations", vec![
                "heat flooding the chest and climbing up the neck",
                "pulse hammering in the ears, drowning out all other sound",
                "a surge of adrenaline that makes the hands shake",
                "muscles tightening across the shoulders and back like armor going on",
                "stomach knotting into a hard, hot fist",
                "blood pounding so hard the vision pulses at the edges",
                "throat constricting, making it hard to get words out",
                "a burning sensation behind the eyes — pressure building",
                "jaw aching from sustained clenching",
                "skin flushing hot, the face and ears going red",
                "a restless electric energy crackling under the skin",
                "the metallic taste of bitten tongue or cheek",
            ]),
            ("Mental Responses", vec![
                "tunnel vision — everything narrowing to the source of the offense",
                "replaying the moment over and over, each replay stoking the fire higher",
                "thoughts racing too fast to finish one before the next ignites",
                "the urge to lash out, to break something, to make someone feel this",
                "snap judgments — everyone becomes an obstacle or an enemy",
                "words sharpening themselves in the mind before they leave the mouth",
                "fantasizing about confrontation, about finally saying the unsaid thing",
                "an inability to hear what the other person is actually saying",
                "the conviction that this rage is righteous, justified, deserved",
                "a desire to be cruel — to find the words that will cut deepest",
                "thoughts becoming brittle, black-and-white — no nuance, no mercy",
                "the sense that if this doesn't find an outlet, something will rupture",
            ]),
            ("Suppression Cues", vec![
                "a smile that doesn't reach the eyes — lips stretched, jaw still tight",
                "voice going carefully, dangerously controlled",
                "white-knuckled grip hidden under the table or in a pocket",
                "breathing measured and deliberate — in through the nose, out slow",
                "changing the subject with too much precision",
                "excusing oneself abruptly, retreating before the mask cracks",
                "a single muscle feathering along the jaw — the only leak",
                "spinning a ring or gripping a pen hard enough to leave dents",
                "answering in clipped, overly polite monosyllables",
                "the door closed an inch too firmly — not slammed, but not gentle",
                "sarcasm delivered as a joke, the venom hidden in the punchline",
                "going very, very still — a predator's stillness, not a calm person's",
            ]),
        ]),
        ("Frustration", "The slow burn of being blocked from a goal — effort meeting resistance without release", vec![
            ("Physical Signals", vec![
                "running both hands through the hair, messing it without noticing",
                "rubbing the temples in slow, hard circles",
                "blowing out a long breath through puffed cheeks",
                "throwing hands up and letting them slap back down against the thighs",
                "pacing in short, tight loops — three steps, turn, three steps, turn",
                "tapping a pen against the desk in an accelerating rhythm",
                "squeezing the bridge of the nose with eyes shut",
                "shoving papers aside or pushing a keyboard back too hard",
                "jaw working side to side as if chewing on the problem",
                "staring at a screen or page without seeing it, eyes glazed",
                "sighing heavily — not once but a pattern, every few minutes",
                "drumming fingers in a restless, irregular beat",
                "yanking at a tie or collar as if the room has gotten smaller",
                "raking a hand down the face from forehead to chin",
                "bouncing a knee under the table in double-time",
                "starting to speak, stopping, starting again with a different word",
                "pressing a fist against the mouth while staring into middle distance",
                "crumpling a piece of paper slowly, methodically, into a tight ball",
            ]),
            ("Internal Sensations", vec![
                "a tightness in the chest that won't ease no matter how deep the breath",
                "heat building behind the eyes — not tears, just pressure",
                "muscles in the shoulders and neck winding tighter by the hour",
                "a headache forming at the base of the skull like a slow vise",
                "the restless, itchy energy of a body that wants to move but has nowhere to go",
                "stomach clenching in a knot that food won't untie",
                "the jaw aching from clenching it without realizing",
                "an exhaustion that has nothing to do with sleep",
                "pulse ticking up just enough to feel it in the wrists",
                "a prickling sensation across the scalp and down the back of the neck",
                "the sensation of being stuck — physically stuck — like pushing against a locked door",
            ]),
            ("Mental Responses", vec![
                "replaying the same problem over and over, finding no new angle",
                "the growing certainty that this should not be this hard",
                "snapping at interruptions — every distraction feels like sabotage",
                "thoughts fragmenting, unable to hold a single thread to completion",
                "an urge to abandon the task entirely and walk away",
                "bargaining with the universe — just let this one thing work",
                "mentally cataloguing everyone and everything that got in the way",
                "the creeping suspicion that the effort has been wasted",
                "thoughts looping: try, fail, try the same way, fail again",
                "difficulty hearing what others are saying — their voices sound far away",
                "a brittle, humorless awareness of the absurdity of the situation",
            ]),
            ("Suppression Cues", vec![
                "forcing a tight smile and saying everything's fine through gritted teeth",
                "deliberately unclenching the hands and laying them flat on the table",
                "taking one long, controlled breath before answering",
                "redirecting the energy into busywork — organizing, cleaning, filing",
                "speaking in a clipped, overly measured cadence",
                "laughing it off with a hollow, too-quick ha",
                "going quiet instead of voicing the complaint — lips pressed shut",
                "leaving the room on a thin excuse — water, bathroom, fresh air",
                "swallowing the sharp reply and answering with something neutral",
                "picking up a glass or coffee cup just to have something to grip",
            ]),
        ]),
        ("Resentment", "Anger that has cooled and hardened into a long-held grievance", vec![
            ("Physical Signals", vec![
                "a smile that goes cold the instant the other person looks away",
                "arms crossing slowly, deliberately, like a gate closing",
                "eyes tracking someone across a room with flat, unblinking attention",
                "the jaw setting at the mention of a particular name",
                "a laugh that arrives a beat too late and doesn't touch the eyes",
                "turning slightly away during conversation — a quarter-turn of dismissal",
                "speaking to everyone in the room except one person",
                "the careful, studied politeness reserved for someone you despise",
                "picking at food when seated near the source of the grudge",
                "a flinch — quickly hidden — when the person touches them",
                "responding to good news about the other person with a tight nod and nothing more",
                "bringing up old grievances mid-conversation like pulling a knife from a drawer",
                "the posture stiffening whenever that voice carries across the room",
                "doing small, deniable things — forgetting to pass a message, arriving late",
                "watching the other person's success with a stillness that could be mistaken for calm",
                "the door closed with exactly enough force to be noticed but not accused",
            ]),
            ("Internal Sensations", vec![
                "a slow, corrosive burn in the chest — not hot like anger, but acid-steady",
                "the stomach knotting at a name, a voice, a memory",
                "tension living permanently in the shoulders like a second skeleton",
                "a bitter taste at the back of the throat that arrives with certain thoughts",
                "the jaw aching from hours of unconscious clenching",
                "a heaviness that settles in after every interaction with the person",
                "the heart rate ticking up at the sound of their footsteps",
                "a low-grade nausea that comes and goes around triggering situations",
                "sleep that won't come, the mind too busy cataloguing old wrongs",
                "the exhaustion of carrying something heavy that nobody can see",
            ]),
            ("Mental Responses", vec![
                "replaying the original offense in perfect, punishing detail — for the hundredth time",
                "mentally rehearsing the speech that will never be given",
                "keeping score — every slight catalogued, every imbalance noted",
                "interpreting neutral actions as further proof of the pattern",
                "fantasizing about the day the other person finally understands what they did",
                "the conviction that forgiveness would be the same as saying it was okay",
                "comparing what they got to what you deserved — the math never balances",
                "noticing every advantage the other person has and feeling the ledger tilt",
                "an inability to hear their name without the whole history flooding back",
                "the quiet, poisonous comfort of being the wronged party",
                "thoughts circling back to the grievance like a tongue returning to a broken tooth",
            ]),
            ("Suppression Cues", vec![
                "answering questions about the person with a shrug and 'it's fine'",
                "changing the subject with practiced ease whenever the topic drifts close",
                "performing friendliness in public — the warmth evaporating the instant they're alone",
                "burying the resentment under productivity, staying too busy to feel it",
                "deflecting with humor that has a barb hidden inside, deniable if called out",
                "agreeing to plans involving the person, then finding reasons to cancel",
                "the careful neutral face — no expression that could be read as hostile",
                "venting to a third party instead of addressing the source",
                "telling themselves they're over it, while the body stays rigid and the sleep stays broken",
                "swallowing the real answer and offering the polite one instead",
            ]),
        ]),
        ("Contempt", "The cold, dismissive feeling of judging someone as beneath you", vec![
            ("Physical Signals", vec![
                "one corner of the mouth lifting into a lopsided smirk — the only asymmetrical human expression",
                "chin raised, eyes lowered — literally looking down the nose",
                "a slow, deliberate eye-roll, not bothering to hide it",
                "head tilted slightly back, exposing the throat — a display of unthreatened superiority",
                "a single eyebrow arched, held there like a question that doesn't expect an answer",
                "waving a hand as if clearing smoke — dismissing the person mid-sentence",
                "the exaggerated sigh, loud enough to be heard across the room",
                "arms folded high on the chest, body angled away",
                "examining fingernails or adjusting a cuff while the other person speaks",
                "a snort — half-laugh, half-dismissal — through the nose",
                "speaking to a third party as though the target isn't in the room",
                "the tongue click followed by a slow head shake",
                "flicking an invisible speck off a sleeve during someone's earnest plea",
                "a smile that arrives with a slight narrowing of the eyes — amusement at someone's expense",
                "pointing with a single finger, then curling it back — a beckoning turned dismissal",
                "leaning back in the chair, one arm draped over the back — taking up space to shrink someone else",
                "the slow blink that says the other person's words weren't worth keeping eyes open for",
            ]),
            ("Internal Sensations", vec![
                "a cold, clean feeling — none of anger's heat, just certainty",
                "the absence of quickened pulse — the body is calm because the threat is beneath it",
                "a tightening at the corners of the mouth, the smirk trying to form",
                "a faint disgust in the stomach, but distant, clinical",
                "the pleasant hum of feeling above it all",
                "jaw relaxed, shoulders down — the body language of someone who doesn't need to brace",
                "a dry, detached clarity, as if watching the other person through glass",
                "the slight lift in the chest that comes with feeling right",
                "an impatience that manifests as stillness — waiting for the lesser person to finish",
                "the cool satisfaction of a verdict already reached",
            ]),
            ("Mental Responses", vec![
                "the settled conviction that this person is not worth the energy of real anger",
                "cataloguing the other person's flaws with quiet precision",
                "the thought forming clearly: I am better than this",
                "mentally reducing the other person to a type — predictable, beneath consideration",
                "an urge to correct, to educate, as if speaking to a child",
                "finding their arguments amusing rather than threatening",
                "the desire to deliver a cutting remark so precise it ends the conversation",
                "wondering why this person is still talking",
                "a refusal to engage seriously — their opinion isn't worth the effort of rebuttal",
                "the belief that they cannot change, that they are beyond repair",
                "ranking the person — socially, morally, intellectually — and placing them below",
            ]),
            ("Suppression Cues", vec![
                "pressing lips together to stop the smirk from fully forming",
                "looking down or away to hide the eye-roll already in motion",
                "replacing the sneer with a neutral, overly patient expression",
                "speaking in a carefully measured, maddeningly calm voice",
                "nodding along while the eyes have already gone flat and empty",
                "offering a compliment so thin it's nearly transparent — 'how brave of you'",
                "redirecting attention to the phone or a document to avoid showing the face",
                "the micro-expression that flashes for a quarter-second before the mask goes on — one lip corner twitching up",
                "answering with monosyllables to avoid saying what's actually being thought",
                "excusing the behavior aloud — 'they don't know any better' — the pity worse than the anger",
            ]),
        ]),
        ("Indignation", "Righteous anger at a perceived injustice that violates a moral principle", vec![
            ("Physical Signals", vec![
                "drawing up to full height — spine straightening, shoulders squaring",
                "chin lifting, jaw set, the posture of someone who knows they are right",
                "voice rising not to a shout but to a ringing, carrying clarity",
                "an open-palmed gesture — hand striking the air as if laying down a verdict",
                "nostrils flaring with each sharp, deliberate breath",
                "eyes widening, then locking on with an unblinking moral focus",
                "stepping forward rather than away — closing distance with purpose, not aggression",
                "a finger pointed not to threaten but to accuse, steady as a compass needle",
                "the head shaking slowly — not confusion, but refusal to accept what was just witnessed",
                "hands planted on hips, elbows out — taking up space in the name of a principle",
                "lips parting to speak, then pressing shut, then parting again — words being chosen like weapons",
                "color rising in the face and neck — the flush of conviction, not embarrassment",
                "slamming a hand flat on the table — not to startle but to punctuate a moral point",
                "marching out of a room with rigid, measured steps — a departure that is itself a statement",
                "holding someone's gaze long past the point of comfort, demanding they look at what they've done",
                "voice dropping to a trembling quiet that is somehow louder than shouting",
            ]),
            ("Internal Sensations", vec![
                "a hot, clean flame in the chest — not the wild fire of rage but a focused burn",
                "the heart beating harder, not faster — each beat deliberate, fortifying",
                "blood rushing to the face and ears in a wave of righteous heat",
                "a tightening across the ribs as if the body is bracing to hold a larger truth",
                "the hands tingling with the urge to act, to intervene, to set something right",
                "a pressure building behind the sternum — the physical weight of something that must be said",
                "the throat opening rather than closing — this emotion wants to speak",
                "adrenaline arriving not as panic but as fuel, sharpening every sense",
                "a quivering in the muscles that feels like readiness rather than fear",
                "the electric clarity of knowing exactly where the line is and that it has been crossed",
            ]),
            ("Mental Responses", vec![
                "the absolute certainty that this is wrong — not an opinion but a fact",
                "thoughts organizing themselves into arguments, evidence, a case for the prosecution",
                "the refusal to let this pass — silence would be complicity",
                "mentally stripping away excuses and rationalizations to expose the bare injustice",
                "an awareness that this anger is not personal — it would burn the same if it happened to a stranger",
                "the urge to speak for those who cannot speak for themselves",
                "a sense of moral clarity so sharp it feels almost physical",
                "the conviction that someone must be held accountable, and if no one else will, then it falls here",
                "thoughts racing ahead to consequences — what must be done, who must be told",
                "the strange, steadying calm that comes from knowing you are on the right side of something",
                "an impatience with anyone trying to minimize or explain away the offense",
            ]),
            ("Suppression Cues", vec![
                "biting the inside of the cheek to keep the accusation from flying out",
                "hands gripping the edge of a chair, channeling the energy downward",
                "a long, controlled exhale through the nose — buying time before speaking",
                "the jaw working silently, words being ground down before they reach the air",
                "looking away to compose the face — but the eyes keep drifting back",
                "responding with clipped, formal precision — every word vetted for professionalism",
                "writing it down instead of saying it — the pen pressing hard enough to score the paper",
                "leaving the room not in retreat but to prevent saying something that can't be taken back",
                "the hands trembling slightly despite the controlled voice",
                "channeling the outrage into pointed, unanswerable questions rather than accusations",
            ]),
        ]),
        ("Bitterness", "Resentment calcified into a worldview — the sense that life has been fundamentally unfair", vec![
            ("Physical Signals", vec![
                "a mouth that has settled into a permanent downturn — not frowning, just no longer bothering to lift",
                "eyes that narrow at good news the way other people's narrow at bad",
                "a laugh that sounds like something scraping — short, dry, humorless",
                "arms crossed not in defense but in verdict, the body already closed to whatever comes next",
                "a slow head shake at optimism, as reflexive as blinking",
                "the lip curling faintly when someone describes their happiness",
                "heavy, theatrical sighs deployed like punctuation",
                "slouching into a chair as though the weight of unfairness is a physical thing",
                "picking at food without appetite, pushing it around the plate",
                "the jaw perpetually tight, the face lined from years of clenching",
                "speaking without looking up, as if the person isn't worth the effort of eye contact",
                "a sarcastic slow clap or mocking toast",
                "fingers tapping impatiently through someone else's good fortune story",
                "the gaze drifting to the middle distance during conversations — already somewhere else, somewhere worse",
                "a smile that looks more like a wince, offered and withdrawn in the same breath",
            ]),
            ("Internal Sensations", vec![
                "a low, constant ache behind the ribs — not sharp enough to demand attention, just always there",
                "the sourness that lives in the back of the throat, activated by certain memories",
                "fatigue that sleep doesn't touch — a weariness rooted in the bones",
                "a tightness in the stomach that has become so familiar it feels like a companion",
                "the dull throb of a headache that arrives with every reminder of what was lost",
                "a heaviness in the limbs, as if the body has absorbed years of disappointment and is now waterlogged with it",
                "the metallic aftertaste of swallowed words, accumulated over years",
                "appetite gone flat — food tastes like nothing, pleasure tastes like nothing",
                "the low-grade tension of muscles that have forgotten how to fully relax",
                "a coldness in the chest where warmth used to live",
            ]),
            ("Mental Responses", vec![
                "the reflex to find the flaw in every silver lining before anyone can point it out",
                "spinning 'if only' scenarios — rewriting the past in an endless, futile loop",
                "the conviction that other people's happiness is evidence of your own bad luck",
                "replaying the story of the original wound for the thousandth time, polished to a bitter shine",
                "dismissing compliments as manipulation or pity",
                "black-and-white thinking — the world divided into people who got what they deserved and you",
                "preemptively expecting disappointment so it can't surprise you anymore",
                "a running internal monologue of grievances, organized by severity and date",
                "the inability to celebrate someone else's win without calculating what it cost you",
                "thoughts that begin with 'must be nice' and end with silence",
                "the quiet, corrosive certainty that happiness is something that happens to other people",
            ]),
            ("Suppression Cues", vec![
                "offering congratulations in a flat voice and changing the subject immediately",
                "masking the bitterness with self-deprecating humor — 'that's just my luck'",
                "withdrawing from social situations rather than letting the sourness show",
                "burying the complaints under a layer of practiced indifference — 'I don't care anyway'",
                "smiling at the right moments but never with the eyes",
                "redirecting conversations away from personal topics to avoid the acid leaking through",
                "keeping interactions short and surface-level — depth is where the bitterness lives",
                "the careful performance of being 'fine' that fools acquaintances but not anyone close",
                "channeling the energy into dark, cutting wit that people mistake for personality",
                "going very still and quiet when triggered, the bitterness swallowed so hard the throat bobs visibly",
            ]),
        ]),
        ("Jealousy", "The threatened, possessive anger of fearing you will lose something to a rival", vec![
            ("Physical Signals", vec![
                "eyes tracking a rival's movements with predatory focus",
                "hovering near a partner — standing too close, a hand on the shoulder, marking territory",
                "jaw clenching at the sound of someone else's laughter directed at the wrong person",
                "arms crossing tightly over the chest, the body building a wall",
                "a smile that dies the instant the rival enters the room",
                "fingers tightening around a glass stem until the knuckles whiten",
                "the posture straightening, shoulders squaring — unconsciously making oneself larger",
                "lips pressing into a thin line while watching an interaction play out",
                "fidgeting — shifting weight, adjusting clothing, unable to settle",
                "mimicking the rival's gestures without realizing it",
                "turning the body slightly away from the scene, as if looking directly would admit too much",
                "reaching for a partner's hand mid-conversation — a tie sign, holding on",
                "the ugly, scorn-filled laugh that comes out instead of a real one",
                "muttering something under the breath, just quiet enough to deny",
                "quick, sharp movements — tossing hair, snatching up a bag, wiping at eyes",
                "the stare that lingers a beat too long on the rival, measuring, comparing",
                "pulling a phone out and pretending to scroll — a retreat disguised as disinterest",
            ]),
            ("Internal Sensations", vec![
                "a cold knot forming in the stomach, tight and hard as a fist",
                "heat climbing the neck and flooding the face — the flush of threat, not embarrassment",
                "the heart rate spiking at an innocent touch between two other people",
                "a queasy, seasick feeling — the ground shifting under something you thought was solid",
                "palms going clammy, the sweat of vigilance",
                "a sharp, physical pang behind the sternum — the sting of being replaced",
                "the throat constricting around words that want to come out as accusations",
                "adrenaline humming through the limbs with nowhere to go",
                "a bitter taste flooding the mouth, as if the emotion has a flavor",
                "the skin prickling with hyperawareness — every sound, every gesture amplified",
                "a hollow, sinking feeling in the chest, as if something is being scooped out",
            ]),
            ("Mental Responses", vec![
                "the compulsive need to know — where they are, who they're with, what was said",
                "replaying a harmless conversation and finding threat in every pause and smile",
                "mentally cataloguing every advantage the rival has — younger, funnier, more interesting",
                "the spiral of comparison: measuring yourself against someone else and always losing",
                "thoughts racing through worst-case scenarios, building a catastrophe from nothing",
                "an urge to test the other person — ask a loaded question, set a small trap",
                "the conviction that noticing is the same as knowing, that the gut feeling is evidence",
                "swinging between wanting to confront and wanting to pretend it's not happening",
                "interpreting every delayed text, every distracted glance, as confirmation",
                "the poisonous thought arriving fully formed: they prefer that person to me",
                "an inability to stop watching, even though watching makes it worse",
            ]),
            ("Suppression Cues", vec![
                "forcing a bright, too-wide smile and saying 'no, go ahead, have fun'",
                "laughing along at the rival's joke, the sound scraping the throat on the way out",
                "uncrossing the arms deliberately, performing relaxation",
                "biting the inside of the cheek to keep the accusation from escaping",
                "changing the subject with manic energy when the conversation veers close",
                "offering excessive, hollow compliments to the rival — overcompensating until it rings false",
                "the phone picked up again, scrolling without reading, needing something for the hands",
                "leaving the room calmly, then gripping the bathroom sink with both hands",
                "telling yourself it's nothing, it's nothing, it's nothing — the mantra of the unconvinced",
                "redirecting the energy into sudden, unnecessary productivity — cleaning, organizing, texting someone else",
            ]),
        ]),
        ("Envy", "The corrosive desire for what someone else has, edged with resentment", vec![
            ("Physical Signals", vec![
                "eyes lingering on what someone else has — the house, the partner, the easy laugh",
                "a smile that tightens at the corners when congratulations are expected",
                "the gaze dropping to one's own hands, one's own clothes, one's own life — and finding them wanting",
                "a slight flinch at someone else's good news, covered immediately",
                "the mouth turning down for just a moment before the compliment is forced out",
                "scrolling past someone's success on a screen, then scrolling back to stare",
                "picking at one's own appearance — adjusting a hem, smoothing hair — after seeing someone effortlessly polished",
                "the posture shrinking, shoulders drawing inward, making oneself smaller in the other's presence",
                "a dismissive wave of the hand — 'oh, that's nice' — delivered with the warmth of a closed door",
                "narrowed eyes tracking someone across a room, measuring everything they have",
                "the jaw setting when someone mentions the other person's accomplishments",
                "fidgeting with a ring or watch while listening to someone describe their life",
                "the too-quick look away when caught staring at what someone else possesses",
                "a laugh that comes out sharper than intended at news of the other's success",
                "crossing arms and leaning back — the body pulling away from what it can't have",
            ]),
            ("Internal Sensations", vec![
                "a pang in the chest — sudden, specific, like a finger pressed on a bruise",
                "a hollowness opening in the stomach, the physical ache of lack",
                "heat creeping up the neck — not anger's bonfire but a slow, shameful warmth",
                "the throat tightening around words of praise that don't want to come out",
                "a sour taste in the mouth, as if the emotion has curdled something",
                "a restless, itching discomfort under the skin — wanting to crawl out of one's own life",
                "the chest constricting, a feeling of being compressed, made less",
                "a dull ache that starts in the sternum and radiates outward like a stain",
                "the stomach dropping when the comparison lands and you come up short",
                "a low hum of tension in the body that doesn't resolve — envy has no release valve",
            ]),
            ("Mental Responses", vec![
                "the involuntary calculation: what they have minus what I have equals what I'm missing",
                "minimizing their achievement — they got lucky, they had connections, it was handed to them",
                "the thought arriving unbidden: why them and not me",
                "fixating on the one thing the other person has that you don't, unable to see anything else",
                "building a case for why they don't deserve it — picking apart their worth to restore your own",
                "the fantasy of having their life, slipping into it like a coat and finding it fits",
                "a bitter awareness that wanting what they have says something ugly about you",
                "thoughts cycling between admiration and resentment, unable to settle on either",
                "the compulsive urge to compare — salary, looks, partner, house, happiness — always losing",
                "telling yourself you don't even want it while the wanting eats at you from the inside",
                "the corrosive question on repeat: what's wrong with me that I don't have this",
            ]),
            ("Suppression Cues", vec![
                "delivering a compliment that sounds right but costs everything to say",
                "asking interested questions about their success while the smile aches with effort",
                "turning the conversation to a neutral topic before the envy can surface",
                "overcompensating with enthusiasm — 'that's SO amazing!' — the volume a tell",
                "focusing intently on a phone or drink to avoid looking at what triggers the feeling",
                "making a self-deprecating joke to preempt the comparison — 'must be nice, meanwhile I can barely...'",
                "channeling the energy into sudden, fierce ambition — if I can't have theirs, I'll build my own",
                "going quiet at a dinner party and hoping no one notices the withdrawal",
                "mentally listing one's own accomplishments like an antidote, trying to balance the scales",
                "excusing yourself to the bathroom to breathe through the tightness in the chest before returning with a fresh face",
            ]),
        ]),
        // ── Fear Family ──
        ("Fear", "The primal alarm that danger is near — the body bracing before the mind understands why", vec![
            ("Physical Signals", vec![
                "eyes blowing wide, whites showing all around the iris",
                "the body going absolutely still — the freeze before the flight",
                "skin draining to the color of old wax",
                "backing away in small, unconscious steps, hands rising as if to ward something off",
                "trembling that starts in the hands and spreads until the whole frame is shaking",
                "flinching at a sound, a shadow, a door opening too fast",
                "breath coming in short, shallow sips — the lungs refusing to fill",
                "sweat breaking out along the hairline and upper lip despite the cold",
                "gripping the nearest solid thing — a doorframe, a railing, someone's arm",
                "the mouth opening and closing without sound, words swallowed by the throat",
                "scanning the room with darting eyes, mapping exits without conscious thought",
                "pressing flat against a wall, making the body as small as possible",
                "hair rising on the nape and forearms — the ancient alarm no amount of evolution erased",
                "a hand flying to the chest or throat, covering what feels most vulnerable",
                "legs locking in place when every instinct screams to run",
                "the jaw dropping open, the face slack and unguarded",
                "hunching the shoulders up around the ears as if bracing for impact",
                "tendons standing out in the neck, the pulse visible and hammering",
                "stumbling backward, tripping over nothing, the body fleeing before the mind agrees",
            ]),
            ("Internal Sensations", vec![
                "the stomach dropping as if the floor has vanished — a freefall feeling with nowhere to land",
                "ice flooding the veins, a cold that starts in the center and radiates outward",
                "the heart slamming against the ribs so hard it feels audible",
                "throat closing to a pinhole — swallowing becomes a conscious effort",
                "legs going liquid, the bones replaced with something that won't hold weight",
                "a jolt of adrenaline that tastes like copper on the tongue",
                "every nerve ending firing at once — the skin electric, hypersensitive to every stimulus",
                "the bowels loosening, the bladder pressing — the body trying to shed weight for flight",
                "a tingling numbness spreading through the fingers and lips",
                "the scalp tightening, every hair follicle contracting",
                "nausea rolling through in a thick, oily wave",
                "the sense that time has slowed to a crawl, each second stretching wide enough to live inside",
                "a ringing in the ears that muffles everything except the heartbeat",
            ]),
            ("Mental Responses", vec![
                "thoughts scattering like startled birds — impossible to catch a single one",
                "the mind calculating escape routes with a speed and clarity it never has otherwise",
                "a single thought repeating on a loop: get out get out get out",
                "catastrophic thinking at full gallop — every outcome the worst possible one",
                "hyper-awareness of every sound, every movement, every shift in light",
                "the conviction that something terrible is about to happen, the certainty of it",
                "time distortion — seconds stretching into minutes, the moment before impact lasting forever",
                "the mind going blank, white, empty — not calm but overloaded to the point of shutdown",
                "bargaining with the universe in fragments: please, not this, not now, not here",
                "the inability to remember how to do simple things — unlock a door, dial a number, form a sentence",
                "a strange, detached clarity that floats above the panic, observing it from a distance",
            ]),
            ("Suppression Cues", vec![
                "hands shoved deep into pockets to hide the trembling",
                "breathing forced into a slow, deliberate rhythm — in for four, hold, out for four",
                "jaw clenched to keep the teeth from chattering",
                "speaking in a carefully level voice that betrays nothing except by its very evenness",
                "gripping one's own wrist behind the back, the squeeze a private anchor",
                "keeping the eyes fixed forward, refusing to look at whatever triggers the fear",
                "swallowing repeatedly, the throat bobbing with the effort",
                "humor deployed as a shield — a weak joke, a shaky laugh, anything to break the tension",
                "busying the hands with a mundane task to stop them from shaking visibly",
                "the stoic mask held in place by sheer will, contradicted only by the pulse hammering at the temple",
                "talking constantly — filling the silence because silence is where the fear lives",
            ]),
        ]),
        ("Anxiety", "Fear without a clear object — a diffuse, persistent sense that something bad is coming", vec![
            ("Physical Signals", vec![
                "picking at cuticles until the skin reddens and peels",
                "bouncing a knee under the table in a rapid, unconscious rhythm",
                "checking the phone, the door, the time — checking, checking, checking",
                "biting the inside of the lip or chewing at the corner of a thumbnail",
                "running a hand through the hair again and again, the gesture becoming a loop",
                "eyes darting to every sound — the creak of a floorboard, a distant car, a voice in the next room",
                "shoulders creeping up toward the ears, held there for hours without noticing",
                "fidgeting with a ring, a button, a zipper pull — the hands needing something to do",
                "swallowing repeatedly, the throat dry no matter how much water",
                "the inability to sit still — standing, sitting, crossing legs, uncrossing, standing again",
                "a tight, quick smile that vanishes the moment no one's looking",
                "speaking too fast, words tumbling over each other, circling back, over-explaining",
                "wrapping arms around the midsection, a self-hug that isn't comforting",
                "the startled flinch at a hand on the shoulder, a phone buzzing, a door closing",
                "repeatedly smoothing clothes, straightening objects, imposing small orders on an uncontrollable world",
                "pacing a room without purpose, pausing at the window, pacing again",
            ]),
            ("Internal Sensations", vec![
                "a tightness in the chest that feels like a belt cinched one notch too far",
                "the heart fluttering — not pounding like fear, but skipping, stuttering, unreliable",
                "a low, constant nausea that never quite tips into sickness",
                "the feeling of not being able to get a full breath, as if the lungs have shrunk",
                "a buzzing under the skin — not adrenaline's spike but a persistent, low-voltage hum",
                "stomach churning slowly, endlessly, like a washing machine on a cycle that won't end",
                "muscles aching from tension held so long the body has forgotten how to release it",
                "a headache building from the base of the skull — the kind that lives between the shoulders",
                "the pins-and-needles sensation in the fingertips and toes, as if the blood has partially retreated",
                "a lump in the throat that swallowing can't clear",
                "the exhaustion of a body running its alarm system on an empty room, all night, every night",
                "a dull pressure behind the eyes, not pain exactly, but a weight",
            ]),
            ("Mental Responses", vec![
                "thoughts spiraling from a minor worry into a worst-case catastrophe in under a minute",
                "the inability to identify what exactly is wrong — just the unshakeable sense that something is",
                "replaying a conversation from hours ago, searching for the thing that was said wrong",
                "a mental checklist that never reaches the bottom — one worry resolved spawns two more",
                "the future telescoping into a series of increasingly terrible what-ifs",
                "difficulty concentrating — reading the same paragraph three times without absorbing a word",
                "the mind snagging on a single worry and returning to it compulsively, like worrying a loose tooth",
                "a feeling of being unprepared for something unnamed, a test with no subject",
                "the irrational certainty that relaxing will cause the bad thing to happen — vigilance as superstition",
                "thoughts racing so fast they blur together, a wheel spinning too quickly to read the spokes",
                "the inability to trust one's own judgment — is this a real problem or am I making it up",
            ]),
            ("Suppression Cues", vec![
                "forcing the hands to lie flat and still on the table, every muscle in the arms fighting it",
                "taking a deep breath and holding it, counting internally before answering",
                "steering the conversation to safe topics with the precision of someone navigating a minefield",
                "making lists — organizing, planning, structuring — because structure feels like control",
                "the bright, brittle 'I'm fine!' that arrives too quickly and at too high a pitch",
                "excusing oneself for air, for water, for a walk — needing to be alone with the feeling",
                "focusing intently on a task, using productivity as a dam against the rising water",
                "gripping a mug with both hands, the warmth and the weight a grounding anchor",
                "nodding along to a conversation while the mind races through contingencies behind the eyes",
                "wearing the calm like a costume — every gesture deliberate, every word pre-screened, exhausting",
            ]),
        ]),
        ("Dread", "Anticipatory terror of a known bad outcome that cannot be stopped", vec![
            ("Physical Signals", vec![
                "a stillness that isn't calm — the body going rigid in the chair, hands flat on the table, not moving, not blinking",
                "the color draining from the face slowly, not the sudden blanch of shock but a gradual leeching, like watching a tide go out",
                "swallowing compulsively, the throat clicking dry, over and over, as if rehearsing for a word that won't come",
                "fingers gripping the edge of a table or armrest with white-knuckled force, anchoring against something that hasn't arrived yet",
                "the eyes going glassy and fixed on a middle distance, seeing not the room but the approaching thing",
                "a slow, involuntary head shake — tiny, almost imperceptible — the body's mute refusal before the mind catches up",
                "breathing that becomes shallow and deliberate, each inhale measured and insufficient, as if deep breaths might invite the thing closer",
                "the jaw setting hard, muscles bunching at the hinge, teeth locked together behind closed lips",
                "pacing with no destination — three steps toward the window, three steps back, the body unable to hold still or commit to a direction",
                "picking up objects and putting them down without purpose — a pen, a glass, a phone — the hands needing something to do",
                "the posture sinking lower by degrees, shoulders rounding, spine curving, the body folding toward the fetal over minutes",
                "skin prickling with gooseflesh despite a warm room, the hair on the forearms rising as if before a storm",
                "lips pressing into a thin, bloodless line, the mouth sealing itself shut against the scream or the plea building behind it",
                "a hand pressing flat against the stomach or chest, holding something in, holding something down",
                "checking the clock or the door repeatedly — quick glances that betray the countdown running in the head",
            ]),
            ("Internal Sensations", vec![
                "a slow, cold heaviness settling in the gut, not the sharp drop of surprise but a weight lowered by degrees like an anchor into dark water",
                "the heart beating with a thick, leaden quality — not racing but pounding, each beat deliberate and too loud, a countdown drum",
                "a tightness across the chest that builds so gradually it's only noticed when breathing becomes difficult",
                "the stomach turning in long, slow rolls, a seasickness with no waves, the nausea of knowing what's coming",
                "a chill that starts at the base of the spine and climbs vertebra by vertebra, unhurried and thorough",
                "cortisol rising in a sustained flood rather than a spike — the body's alarm set not to a siren but to a slow, unrelenting drone",
                "the mouth going dry, tongue thick and tacky, swallowing producing nothing",
                "a metallic taste seeping onto the back of the tongue, the flavor of adrenaline with nowhere to go",
                "muscles locking into a low-grade tension that won't release — the shoulders, the neck, the jaw all quietly clenching and holding",
                "the hands going cold while the core stays warm, blood quietly redistributing for a fight that can't be fought",
                "a buzzing numbness creeping into the fingertips and lips, as if the body is beginning to shut down its periphery",
                "the sensation of time thickening — each second stretching, elastic and cruel, the clock moving through honey",
            ]),
            ("Mental Responses", vec![
                "the mind running the scenario forward to its conclusion, over and over, unable to stop the projection even when the ending is always the same",
                "a desperate arithmetic of time — how many minutes, how many hours, counting down to the thing that cannot be stopped",
                "the irrational conviction that thinking about it enough might somehow change the outcome, the mind mistaking attention for control",
                "thoughts narrowing to a single point, all peripheral concerns falling away until only the approaching thing remains",
                "bargaining with the universe in fragments — if I don't look, if I hold my breath, if I stay perfectly still",
                "the strange clarity that arrives alongside dread, every detail of the room becoming hyper-sharp, over-lit, too real",
                "a looping rehearsal of the worst moment — what will be said, what expression will be worn, how the blow will land",
                "the awareness that others are still laughing, still eating, still living in a timeline where the thing hasn't happened yet",
                "a furious envy of the person you were an hour ago, before you knew",
                "the mind offering escape fantasies — running, vanishing, waking up — each one examined and discarded in seconds",
                "an exhaustion born not from effort but from the sustained attention to approaching pain, the neural cost of bracing",
                "the paradoxical wish for it to just happen already — the willingness to accept greater pain to end the waiting",
            ]),
            ("Suppression Cues", vec![
                "maintaining a conversation on autopilot, words emerging in the right order while the mind is entirely elsewhere",
                "smiling at a joke a full beat too late, the delay visible only to someone watching closely",
                "the careful, controlled movements of someone pretending the world hasn't tilted — picking up a cup, sipping, setting it down",
                "deflecting questions about the obvious pallor with a wave — 'just tired, didn't sleep well' — the excuse pre-loaded and ready",
                "focusing with fierce, artificial intensity on something trivial — the pattern of a tablecloth, the arrangement of objects on a shelf",
                "the voice kept level through sheer mechanical effort, each word spoken at a measured pace to prevent the tremor underneath",
                "checking a phone repeatedly, pretending the glances are casual, manufacturing a reason to look away",
                "making plans for after — dinner, errands, next week — as if speaking a normal future into existence could make it true",
                "the controlled exhale through the nose, the only visible sign of the breathing exercises happening behind a still face",
                "busying the body with practical tasks — cleaning, organizing, preparing — channeling the unbearable waiting into the illusion of usefulness",
            ]),
        ]),
        ("Horror", "Fear combined with moral revulsion at something that violates the natural order", vec![
            ("Physical Signals", vec![
                "the whole body going rigid — not fear's flinch but a total lock, muscles turned to stone",
                "a hand rising slowly to cover the mouth, the gesture involuntary and ancient",
                "eyes wide and fixed, unable to look away even as every instinct screams to",
                "the head shaking side to side in small, unconscious refusals — no, no, no",
                "skin going the color of ash, the blood retreating from the surface",
                "stumbling backward, legs moving before the brain catches up",
                "retching — a dry, convulsive heave from deep in the gut",
                "the jaw dropping open, lips pulling back from the teeth in a grimace that is half-scream",
                "hands rising to the sides of the head, fingers gripping the hair, pulling",
                "the body folding forward at the waist as if the sight has landed a physical blow to the stomach",
                "trembling that starts in the core and radiates outward until the whole frame vibrates",
                "pressing both palms flat against a wall, needing something solid because the world has tilted",
                "the voice emerging as a whisper when a scream was intended",
                "legs giving way — sinking to the knees, not in grief but in the body's absolute refusal to stay upright",
                "clutching at another person's arm or shoulder, fingernails digging in without awareness",
                "breath stopping entirely for several seconds before returning in a ragged, shuddering gasp",
            ]),
            ("Internal Sensations", vec![
                "the stomach lurching violently, bile climbing the back of the throat",
                "a cold that doesn't start in the veins but in the marrow — deep, structural, wrong",
                "the heart seeming to stop before hammering back to life at double speed",
                "every nerve ending going quiet at once, as if the body has pulled its own circuit breaker",
                "the scalp contracting, the skin tightening across the entire body like something is trying to crawl off the skeleton",
                "a ringing in the ears that swallows all other sound",
                "a taste in the mouth — metallic and sour — that arrives with no physical source",
                "the sensation of the ground tilting, vertigo born not from height but from wrongness",
                "limbs going numb, heavy, useless — the body's shutdown protocol engaging",
                "the visceral, gut-level recognition that what is being witnessed should not exist",
                "a feeling of contamination, as if seeing it has let something in that can't be unseen",
            ]),
            ("Mental Responses", vec![
                "the mind refusing to process what the eyes are reporting — a gap, a stutter, a white blank",
                "the thought arriving not as words but as a full-body understanding: this is wrong",
                "a desperate, scrambling search for an explanation that would make this normal, finding none",
                "the sense that the world has broken a rule, that something fundamental has been violated",
                "time fracturing — the moment stretching into something endless and inescapable",
                "thoughts reduced to fragments: no, this can't, how could, no",
                "an overwhelming urge to undo what has been seen, to rewind, to make it not have happened",
                "the strange, detached clarity of shock — noticing small, irrelevant details with perfect precision",
                "the knowledge that this image will stay, that it has already burned itself into permanent memory",
                "a collapse of the boundary between what the world should be and what it apparently is",
            ]),
            ("Suppression Cues", vec![
                "forcing the eyes shut and pressing the heels of the hands against them",
                "swallowing the bile down, again and again, jaw locked against the retching",
                "turning away with deliberate effort, as if pulling free of something with physical weight",
                "speaking in a flat, affectless monotone — the voice gone blank because feeling anything would break the seal",
                "focusing on a single mundane detail — a crack in the floor, a button on a shirt — as an anchor against the overwhelming",
                "breathing through the mouth in slow, controlled pulls to keep the nausea at bay",
                "gripping one's own arms, fingernails leaving crescent marks, the pain a grounding mechanism",
                "insisting 'I'm fine' in a voice that sounds like it's coming from underwater",
                "going eerily calm and functional, the horror boxed away somewhere the body can't reach yet — the breakdown postponed, not prevented",
                "laughing — a single, broken sound that has nothing to do with humor and everything to do with a mind reaching its limit",
            ]),
        ]),
        ("Paranoia", "The distorted certainty that unseen threats are watching, judging, or plotting", vec![
            ("Physical Signals", vec![
                "eyes scanning every room on entry — the exits, the corners, the faces that look away too quickly",
                "sitting with the back to the wall, never to a door",
                "checking locks — once, twice, a third time, the hand returning to the deadbolt as if pulled by a string",
                "flinching at a phone vibrating, a knock on the door, a car slowing down outside",
                "speaking in a lowered voice even when no one else is near",
                "peering through curtains with two fingers, the movement quick and furtive",
                "jaw clenched perpetually, the muscles knotted from weeks of grinding",
                "darting eyes that never settle — tracking every person who passes, every movement in the periphery",
                "turning around on an empty street to check if someone is following",
                "covering the mouth when speaking, as if someone might read the lips",
                "angling a phone screen away from windows, from other passengers, from cameras",
                "the startle response firing at nothing — a door closing down the hall, a branch against a window",
                "fidgeting with keys held between the knuckles, configured as a weapon",
                "taking different routes home, varying the pattern so no one can learn it",
                "watching someone's hands during conversation, not their face",
                "stepping to the side of a door before opening it, the body flat against the wall",
            ]),
            ("Internal Sensations", vec![
                "a constant low-grade hum of adrenaline that never fully drains — the system stuck in 'on'",
                "the heart lurching at ordinary sounds — the creak of a house settling, a text notification",
                "exhaustion that lives in the marrow, the body worn out from never standing down",
                "muscles perpetually braced, the shoulders and neck a solid knot",
                "a prickling at the back of the neck — the animal certainty of being watched",
                "the stomach tight and sour from days of shallow eating and constant vigilance",
                "a buzzing restlessness that won't let the body be still or the mind be quiet",
                "pupils dilated even in well-lit rooms, the eyes refusing to relax their aperture",
                "the skin crawling with the sensation of exposure, as if standing naked in a crowd",
                "sleep arriving only in shallow, fitful scraps — the deeper layers refused because they require trust",
            ]),
            ("Mental Responses", vec![
                "reading hidden meaning into a coworker's pause, a friend's changed tone, a stranger's glance",
                "assembling evidence from coincidences — the same car twice, a number repeating, a silence in a conversation",
                "the unshakeable conviction that what others call 'normal' is actually a pattern they're too blind to see",
                "mentally rehearsing escape plans, confrontation scripts, worst-case responses",
                "dissecting a text message word by word, finding the threat encoded in what wasn't said",
                "trusting no one fully — even allies are potential liabilities, potential informants",
                "the logic feeling airtight from the inside, every counterargument further proof of the conspiracy",
                "monitoring who talks to whom, who laughed at what, who left the room when you arrived",
                "the thought circling: they know, they already know, they're just waiting",
                "a hyperawareness of being perceived — every interaction a performance under hostile observation",
                "the inability to accept a simple explanation when a sinister one is available",
            ]),
            ("Suppression Cues", vec![
                "forcing the eyes to stop scanning, fixing them on a single neutral point",
                "laughing at one's own jumpiness — 'sorry, I'm just tired' — the excuse deployed like a shield",
                "resisting the urge to check the lock again by gripping the doorknob and making the hand let go",
                "keeping the suspicions internal, performing normalcy while the mind races behind the mask",
                "agreeing that it's probably nothing, while mentally filing the observation for later analysis",
                "deliberately unclenching the jaw, rolling the shoulders, mimicking a body at ease",
                "putting the phone face-down to stop checking it, then picking it up thirty seconds later",
                "telling a therapist or friend a sanitized version — enough to seem cooperative, not enough to be vulnerable",
                "channeling the vigilance into something socially acceptable — security systems, research, planning",
                "the careful, calibrated disclosure: mentioning one concern to test the reaction before revealing the rest",
            ]),
        ]),
        ("Vulnerability", "The exposed, unshielded feeling of having let someone see the parts of you that can be hurt", vec![
            ("Physical Signals", vec![
                "eyes struggling to hold contact — lifting, dropping, lifting again, each return an act of courage",
                "arms wrapping around the midsection, hands gripping elbows, the body trying to hold itself together",
                "voice dropping to barely above a whisper, the words offered rather than projected",
                "palms turning upward in the lap — the wrists exposed, the most unguarded posture the body knows",
                "chin dipping, the throat covered by the instinctive downward tilt of the head",
                "hands trembling slightly while reaching out — to touch, to offer, to give something that can be refused",
                "the shoulders drawing up and inward, shrinking the body's profile",
                "fidgeting with a hem, a ring, a button — the hands needing something to do besides hang open and defenseless",
                "a shaky exhale before speaking, the breath let go like a door being unlocked",
                "tears arriving not from grief but from the sheer exposure of being seen — the crying that surprises even the one doing it",
                "sitting with legs pulled up, chin on the knees, making the body as small as it can go",
                "the tentative, half-finished gesture — a hand extended then pulled back, a step forward then a hesitation",
                "biting the lip hard, the face working to stay composed while the voice breaks",
                "standing in a doorway rather than entering, as if full commitment to the room is too much",
                "the slow, deliberate uncrossing of arms — a conscious decision to stop armoring up",
            ]),
            ("Internal Sensations", vec![
                "the chest feeling cracked open, the ribs parted, everything inside on display",
                "a fluttering in the stomach that is neither excitement nor nausea but something between — the body's register of risk",
                "skin prickling with the hyperawareness of being watched, being weighed, being seen",
                "the throat aching with the effort of pushing words past the instinct to stay silent",
                "a warmth spreading through the chest when the vulnerability is received — the relief of being held instead of dropped",
                "the heart beating louder than it should, as if it knows the armor has been removed",
                "a lightness in the head, almost a dizziness — the vertigo of standing without walls",
                "the physical sensation of exposure — like stepping from a warm building into open wind",
                "a tightness in the solar plexus, the body's core bracing for impact",
                "the strange, simultaneous pull of wanting to run and wanting to stay",
            ]),
            ("Mental Responses", vec![
                "the thought arriving with full force: they can hurt me now, and I have let them",
                "an urge to take it back — to laugh it off, to say 'never mind,' to rebuild the wall mid-sentence",
                "the voice in the head cataloguing every way this could go wrong",
                "a desperate, searching read of the other person's face — are they judging, are they softening, are they leaving",
                "the realization that being seen fully is both the thing most wanted and most feared",
                "thoughts stripped of their usual cleverness, reduced to simple, undefended truths",
                "the awareness that this moment cannot be undone — what has been shown cannot be unshown",
                "a fierce internal argument: this is weakness versus this is the bravest thing you've done",
                "the mind cycling between 'I shouldn't have said that' and 'I had to say that'",
                "the rawness of having no rehearsed answer, no script, no performance — just what is",
            ]),
            ("Suppression Cues", vec![
                "the joke deployed like a drawbridge — pulling up just as the conversation gets real",
                "arms crossing again, the armor going back on mid-confession",
                "voice hardening suddenly, the softness replaced by a clipped, businesslike tone",
                "deflecting with a question: 'but what about you?' — turning the spotlight away",
                "laughing too quickly after admitting something real, the sound meant to shrink the admission",
                "adding 'it's not a big deal' or 'I don't know why I said that' immediately after the disclosure",
                "standing up, moving, putting physical distance between the self and the moment of exposure",
                "retreating into irony — wrapping the truth in enough sarcasm that it can be denied if needed",
                "going silent after revealing too much, the withdrawal as sudden as a door closing",
                "wiping the eyes roughly and straightening the spine — the visible reassembly of composure, piece by piece",
            ]),
        ]),
        // ── Sadness Family ──
        ("Sadness", "The quiet, heavy acknowledgment of loss — the feeling that something good is gone", vec![
            ("Physical Signals", vec![
                "shoulders rounding forward, the body curling in on itself like a question mark",
                "movements slowed to half-speed, as if the air has thickened to water",
                "eyes downcast, focused on nothing — a middle-distance stare that sees inward",
                "the bottom lip trembling before the teeth catch it and hold it still",
                "sitting very still for a long time, hands resting in the lap, forgotten",
                "a deep, shuddering sigh that seems to empty the lungs completely",
                "wiping the eyes with the heel of the hand in a rough, impatient gesture",
                "food pushed around a plate, rearranged but never lifted to the mouth",
                "voice gone quiet and flat, the pitch dropped to a monotone",
                "withdrawing from a group — choosing the chair at the edge, the corner of the couch",
                "the face crumpling for just a moment before being pulled back into composure",
                "hugging a pillow, a jacket, one's own arms — anything that mimics being held",
                "moving through familiar tasks on autopilot, eyes glazed, hands mechanical",
                "lying on a side, knees drawn up, staring at a wall",
                "the slow blink that takes too long to reopen, as if the eyes are reluctant to see again",
                "letting a phone ring without answering, watching the screen until it stops",
            ]),
            ("Internal Sensations", vec![
                "a heaviness in the chest as if something has been placed on the sternum and left there",
                "the lump in the throat that swallowing can't shift — dense, immovable, aching",
                "limbs weighted with a fatigue that has nothing to do with exertion",
                "a hollowness behind the ribs, as if a room inside the chest has been emptied of its furniture",
                "eyes burning with a pressure that may or may not become tears",
                "the dull, whole-body ache of grief — not localized, not sharp, just everywhere",
                "a tightness around the jaw and chin from holding the face steady",
                "the stomach clenched shut, rejecting the idea of food",
                "cold hands, cold feet — the warmth retreating to somewhere unreachable",
                "an exhaustion so deep it feels structural, as if the bones themselves are tired",
                "the strange, physical sensation of the world being less — colors muted, sounds distant, textures flat",
            ]),
            ("Mental Responses", vec![
                "replaying the last good moment, turning it over and over like a stone worn smooth",
                "the thought arriving on a loop: I miss this, I miss this, I miss this",
                "a difficulty concentrating — reading the same line, losing the thread of a conversation",
                "questioning whether the happiness was real, or if it was always leading here",
                "the future collapsing into a flat, gray expanse without features or landmarks",
                "withdrawing from plans, from people, from anything that requires the energy of pretending",
                "a desire to sleep not for rest but for absence — to not be conscious for a while",
                "thinking in fragments rather than full thoughts, the mind unable to sustain its own weight",
                "the sudden, sharp pang triggered by an ordinary thing — a song, a scent, a day of the week",
                "self-blame arriving uninvited: if I had done something differently",
                "the slow realization that the world is continuing as if nothing has changed, when everything has",
            ]),
            ("Suppression Cues", vec![
                "the bright, fragile smile that requires visible effort to maintain",
                "clearing the throat before speaking, forcing the voice up from its monotone",
                "staying busy — cleaning, organizing, moving — because stillness is where the sadness waits",
                "deflecting concern with 'I'm fine' in a voice that dares no one to ask again",
                "laughing at something that isn't funny, the sound thin and unconvincing",
                "excusing red eyes as allergies, tiredness, the weather",
                "biting the inside of the cheek hard enough to taste copper — pain as a dam against tears",
                "turning away at the exact moment the composure starts to crack",
                "focusing on someone else's problem, redirecting care outward to avoid facing inward",
                "the bathroom visit that lasts too long — the locked door, the running tap to cover the sound",
            ]),
        ]),
        ("Grief", "Sadness at full intensity after a devastating loss — a process that moves in waves", vec![
            ("Physical Signals", vec![
                "the body folding in half, arms wrapped around the stomach as if holding the self together",
                "a sound coming out that doesn't sound human — a keen, a wail, something from deep in the animal body",
                "rocking back and forth, the rhythm involuntary, the body soothing itself the way it would a child",
                "hands covering the face, fingers pressing hard into the eye sockets",
                "the face crumpling — not gradually but all at once, every muscle surrendering simultaneously",
                "reaching for the phone to call the person who is gone, the hand stopping mid-dial",
                "moving through a room and touching the dead person's things — a coffee mug, a jacket, a book left open",
                "the sudden inability to stand, legs buckling, the body finding the floor",
                "sitting in the car in the driveway, unable to get out, unable to go in",
                "premature aging arriving in weeks — the gray hair, the deepened lines, the hollowed eyes",
                "staring at nothing for so long that someone has to say the name twice to break through",
                "walking into a room and forgetting why, standing in the doorway, then walking out again",
                "sobbing that comes in waves — subsiding to quiet, then surging back without warning",
                "holding an object that belonged to them and pressing it against the chest, the cheek, the face",
                "the shuffling, effortful gait of someone carrying an invisible weight",
                "sleeping in their clothes, on the couch, in the wrong room — sleep happening where the body falls",
            ]),
            ("Internal Sensations", vec![
                "a physical pain in the chest so acute it could be mistaken for a heart attack — the broken heart that is not a metaphor",
                "the throat closing around a knot so dense that breathing becomes a conscious, labored effort",
                "a hollowness so vast it has its own gravity, pulling everything inward",
                "the body heavy as wet sand — limbs that won't lift, a head that won't raise",
                "waves of nausea that have nothing to do with the stomach and everything to do with the soul",
                "cortisol flooding the system for months — the immune response collapsing, the sleep cycle shattered",
                "a bone-deep exhaustion that sleep cannot repair, as if the body is mourning at the cellular level",
                "the lungs refusing to take a full breath, each inhale catching halfway",
                "a coldness in the core that no blanket or fire can reach",
                "the physical sensation of absence — the empty side of the bed, the missing weight in the room, the silence where a voice should be",
                "dizziness arriving without warning, the world tilting as the ground rearranges itself around the loss",
            ]),
            ("Mental Responses", vec![
                "forgetting they are dead — the first bright seconds of waking before the memory crashes back",
                "the mind looping on final conversations, final touches, final words — rewriting them, bargaining with them",
                "an inability to think past the next hour, the future foreshortened to nothing",
                "seeing them everywhere — in a crowd, in a passing car, in the posture of a stranger who walks the same way",
                "the bizarre, hyper-specific details the mind fixates on instead of the larger truth: the color of the hospital wall, the shape of a signature, the receipt still in a pocket",
                "a fog descending on memory and concentration — names lost, tasks abandoned, sentences trailing off",
                "the guilt arriving: I should have called, I should have been there, I should have said it when I could",
                "the cruel, involuntary thought: what if I forget their voice, their face, the exact way they laughed",
                "time distorting — some hours stretching into weeks while whole days vanish without a trace",
                "the mind refusing to use past tense, tripping over verbs, correcting itself mid-sentence",
                "anger arriving without warning — at the doctors, at God, at the dead person for leaving",
            ]),
            ("Suppression Cues", vec![
                "the jaw set, the spine rigid, the whole body locked into a posture of composure that trembles at the edges",
                "speaking in a measured, careful voice about 'arrangements' and 'logistics' — the language of management replacing the language of loss",
                "keeping busy with tasks — the dishes, the paperwork, the phone calls — because stopping means feeling",
                "smiling at the funeral, receiving condolences with grace, shattering only once the door closes",
                "wearing sunglasses indoors to hide the evidence",
                "saying 'I'm doing okay, really' in a voice that is steady only because every ounce of will is holding it there",
                "redirecting the conversation to the practical — what needs to be done, who needs to be told — safe terrain",
                "gripping a tissue in the fist but refusing to use it, the act of crying itself too large a concession",
                "eyelids fluttering rapidly when the subject comes up — the inner turmoil leaking through the only crack in the mask",
                "the lip purse held for just a moment before composing the face to neutral — the body's private regrouping before returning to the performance of being fine",
            ]),
        ]),
        ("Melancholy", "A gentle, reflective sadness — more like a mist than a storm, often tinged with beauty", vec![
            ("Physical Signals", vec![
                "gazing out a rain-streaked window without seeing the street below",
                "a half-smile that arrives at a memory and fades without becoming anything more",
                "chin resting on a hand, the body settling into stillness rather than collapsing into it",
                "turning the pages of a photo album slowly, fingertips hovering over faces",
                "sighing — not the sharp exhale of frustration but a long, soft release, almost musical",
                "walking alone at dusk, pace unhurried, hands in pockets or trailing along a railing",
                "letting a song play on repeat, each listen pulling the feeling deeper rather than resolving it",
                "eyes soft and slightly unfocused, turned inward, the gaze seeing something that isn't in the room",
                "tracing the rim of a glass with a fingertip, the gesture idle and hypnotic",
                "sitting in a favorite chair long after the light has changed, not bothering with the lamp",
                "re-reading an old letter, the paper handled with the care of something that can't be replaced",
                "picking up a book, reading the same paragraph twice, setting it gently down again",
                "leaning against a doorframe, watching a room where someone used to be",
                "the quiet act of putting on a loved one's old sweater, not for warmth but for the scent",
                "speaking less, but more softly — the voice dropped to a register just above thought",
            ]),
            ("Internal Sensations", vec![
                "a soft ache behind the sternum — not sharp enough to be pain, more like a bruise pressed gently",
                "the body feeling heavy but not depleted — a pleasant weight, like being wrapped in something warm",
                "a tightness in the throat that doesn't demand tears, just sits there like a held note",
                "the curious warmth that comes with remembering something beautiful that is over",
                "a drowsiness of the spirit — not tired but subdued, the inner volume turned low",
                "the eyes stinging faintly, not from crying but from the nearness of crying",
                "a slow pulse of feeling that rises and recedes like a tide — never crashing, never fully retreating",
                "the skin sensitive to minor things — the texture of a blanket, the coolness of air from a cracked window",
                "a sweetness tangled in the sadness, the two so close together they can't be separated",
                "the sense of time passing visibly — watching a candle burn down, a shadow lengthen, a season turn",
            ]),
            ("Mental Responses", vec![
                "thoughts drifting to people and places that exist now only in memory",
                "the awareness that this moment, too, will pass — and finding the ache of that strangely beautiful",
                "a willingness to sit inside the feeling rather than escape it, as if the sadness itself has something to teach",
                "remembering not the facts of a time but its quality — the light, the weather, the way it felt to be there",
                "the philosophical turn: wondering if happiness is always shadowed by impermanence, and whether that's what gives it weight",
                "a gentle inventory of losses — not the catastrophic ones but the quiet erosions: friendships that faded, rooms left behind, versions of the self outgrown",
                "the thought arriving clearly: I was happy then, and I didn't know it",
                "a tenderness toward the world that feels almost unbearable — the beauty of ordinary things seen through the lens of their transience",
                "the mind reaching for a person, a place, a feeling — and finding only the shape of where it used to be",
                "a desire for solitude that isn't loneliness but its opposite — wanting to be alone with the fullness of the feeling",
            ]),
            ("Suppression Cues", vec![
                "shaking the mood off with a brisk movement — standing, straightening, turning on a light",
                "switching to something upbeat — a different song, a brighter room, a conversation that stays on the surface",
                "dismissing the feeling with 'I'm just being sentimental' and a self-conscious laugh",
                "reaching for the phone, for noise, for distraction — anything to break the spell of quiet",
                "pulling away from the window, the album, the letter — closing the door on the reverie before it deepens",
                "joining a group with deliberate energy, the brightness turned up like a dial to cover the softness underneath",
                "making a joke about the rain, the old song, the faded photograph — wrapping the tenderness in armor",
                "getting busy with something practical — dishes, laundry, a reply to an email — hands moving to outpace the heart",
                "saying 'anyway' to close a thought before it reaches the tender part",
                "swallowing the feeling and offering a steady, even smile that says nothing is wrong because nothing dramatic is",
            ]),
        ]),
        ("Loneliness", "The ache of connection missing — feeling invisible or unreachable even in a crowd", vec![
            ("Physical Signals", vec![
                "scrolling through a phone contacts list and finding no one to call",
                "eating standing up at the kitchen counter, not bothering with a plate",
                "talking to a pet with the cadence and detail normally reserved for a person",
                "leaving the television on for the sound of human voices in an empty room",
                "hovering at the edge of a group, laughing a beat too late, never quite joining",
                "setting the table for one — the single placemat, the single glass, the economy of it",
                "checking the mailbox, the inbox, the phone — the small, hopeful rituals of waiting to be remembered",
                "sleeping on one side of the bed, the other half undisturbed for weeks",
                "the apartment growing messy in ways it wouldn't if anyone else could see it",
                "wearing headphones in public, building a wall of sound against the silence of not being spoken to",
                "lingering at a checkout counter, a coffee shop, anywhere a brief exchange might happen",
                "hugging a pillow at night, the arms closing around a substitute for a body",
                "waving back at someone who was waving to the person behind",
                "flinching at one's own voice after a long silence — the sound strange, unused",
                "opening a group chat and typing something, then deleting it, then closing the app",
            ]),
            ("Internal Sensations", vec![
                "a hollow ache in the chest that arrives most acutely at dusk and doesn't leave until sleep",
                "the physical sensation of being untouched — skin hungry, the nerve endings forgetting what contact feels like",
                "a coldness that isn't temperature but absence — the missing warmth of another body in the room",
                "the throat tightening when hearing friends laugh together from a distance",
                "a fatigue that deepens on weekends and holidays, when the world pairs off and the solitude thickens",
                "the stomach clenching at the phrase 'what are you doing tonight?' because the honest answer is nothing",
                "a low-grade inflammation of the spirit — the body's stress response running quietly, persistently, with no threat but the emptiness itself",
                "the strange soreness of a jaw that hasn't spoken in hours, the muscles stiff from disuse",
                "a heaviness settling in the late afternoon, the hours between work and sleep yawning open with nothing to fill them",
                "the physical jolt of a phone buzzing — hope spiking — followed by the deflation of an automated message",
            ]),
            ("Mental Responses", vec![
                "the arithmetic of social connection: counting days since the last real conversation, the last uninitiated text, the last time someone chose your company",
                "wondering if disappearing would be noticed, and how long it would take",
                "rehearsing conversations that never happen — what you'd say if someone asked how you really are",
                "the creeping suspicion that the problem is you — that something fundamental is wrong with you, something that repels",
                "watching other people's ease with each other and not understanding how they do it",
                "the self-fulfilling withdrawal: too lonely to reach out, too proud, too afraid of the silence that might answer",
                "fantasizing about a knock on the door, an unexpected invitation, a coincidence that breaks the pattern",
                "mentally composing messages to old friends and never sending them",
                "the bitter awareness that being surrounded by people and being known by them are two different things entirely",
                "wondering when 'alone' became 'lonely' — trying to locate the exact moment the solitude soured",
            ]),
            ("Suppression Cues", vec![
                "filling every hour with activity — gym, errands, podcasts, anything to avoid the quiet",
                "performing independence as a lifestyle: 'I actually prefer being alone'",
                "curating social media to look connected — posting, commenting, maintaining the performance of a full life",
                "volunteering for overtime, extra shifts, weekend work — the office at least has other voices",
                "adopting a brisk, self-sufficient tone when asked about weekend plans: 'oh, just catching up on things'",
                "keeping the apartment immaculate when guests might visit, letting it collapse the moment they leave",
                "turning down invitations preemptively rather than risking the humiliation of wanting them too much",
                "making friends with baristas, cashiers, regulars — cultivating micro-connections in place of real ones",
                "laughing off the question 'are you seeing anyone?' as if the answer doesn't sit in the chest like a stone",
                "going to a movie alone and choosing the late showing, when the theater is too dark for anyone to notice",
            ]),
        ]),
        ("Despair", "Sadness stripped of hope — the belief that nothing will improve and suffering is permanent", vec![
            ("Physical Signals", vec![
                "the body gone slack — not curling inward like sadness but simply ceasing to hold itself up",
                "sitting on the edge of the bed, feet on the floor, unable to complete the act of standing",
                "staring at a wall, a ceiling, a point in space — the eyes open but no longer searching for anything",
                "hands lying palm-up in the lap, the fingers uncurled, the gesture of surrender",
                "the face gone still and flat, the muscles no longer bothering to compose an expression",
                "letting the phone ring, the door knock, the email ping — none of it worth the motion of responding",
                "clothes worn for the third day, the fourth, the concept of 'changing' belonging to a future that no longer exists",
                "food left untouched on the counter where someone placed it — the body refusing even the labor of hunger",
                "lying on a floor, not a bed — as if the effort of reaching a mattress was the last thing that exceeded capacity",
                "the voice, when it comes, flat and stripped of inflection — a monotone that has stopped performing for anyone",
                "movements reduced to the barest minimum — a hand lifting to push hair from the eyes, then falling back",
                "the slow, shuffling walk of someone who has nowhere to go and no reason to arrive",
                "tears that fall without sound, without expression, without the convulsive effort of crying — just leaking",
                "sitting in a running shower long after the water has gone cold",
                "the thousand-yard stare — eyes focused on a distance that has nothing to do with the room",
            ]),
            ("Internal Sensations", vec![
                "a heaviness so total it feels geological — not weight on the body but weight in the bones, in the marrow",
                "the chest not aching but empty — the pain has burned through to a hollow that feels like nothing at all",
                "breathing that continues only because the body hasn't been given permission to stop",
                "a numbness spreading outward from the center, the nervous system dimming like lights going off room by room",
                "the physical sensation of sinking — not falling but settling, the body finding its lowest possible point",
                "an exhaustion beyond sleep — the kind that makes the concept of 'rest' feel like a language you no longer speak",
                "the stomach not clenched but vacant, the hunger signal simply turned off",
                "the body temperature dropping, the skin going cool and slack, the blood moving as if it too has lost interest",
                "a silence inside where the inner voice used to be — not peace but absence, the radio gone to static",
                "the sensation that the body is very far away, operating on a delay, the self watching from behind glass",
            ]),
            ("Mental Responses", vec![
                "the thought arriving not with anguish but with a terrible calm: it will not get better",
                "the complete cessation of planning — the future is not dark, it is simply gone, a blank page the mind refuses to write on",
                "the 'why bother' that is not rhetorical but genuine — an honest question for which there is no answer",
                "past efforts replaying not as failures but as proof of a fixed equation — effort in, nothing out, always",
                "the mind no longer bargaining, no longer arguing, no longer reaching — just still",
                "thoughts arriving in fragments that don't connect: the ceiling is white, it's Tuesday, nothing matters",
                "the strange clarity of having nothing left to lose — a kind of terrible freedom",
                "wondering without urgency whether this is what the rest of life will feel like",
                "the inability to imagine a version of tomorrow that is different from today",
                "the letting go that is not release but collapse — not choosing to stop fighting but discovering the fight has already ended",
            ]),
            ("Suppression Cues", vec![
                "saying 'I'm just tired' — because exhaustion is an acceptable explanation and despair is not",
                "going through motions with mechanical precision — showering, dressing, commuting — the body on autopilot while the self is elsewhere",
                "answering 'how are you' with 'fine' in a voice so flat it dares anyone to believe it",
                "agreeing to plans with no intention of keeping them — the path of least resistance",
                "smiling when required, the expression assembled from memory rather than feeling, and not quite right",
                "keeping the door closed, the lights off, the curtains drawn — minimizing the surface area that others can see",
                "deflecting concern by turning the conversation practical: 'I just need to figure out a few things'",
                "performing small, visible acts of normalcy — making coffee, checking the weather — as evidence of functioning",
                "sleeping as an escape rather than a rest — choosing unconsciousness over the waking blank",
                "the carefully composed text message: 'all good here :)' — the smiley face doing the work the words cannot",
            ]),
        ]),
        ("Disappointment", "The deflating feeling when reality fails to match expectation", vec![
            ("Physical Signals", vec![
                "shoulders dropping visibly, the posture deflating like something punctured",
                "a long exhale through pursed lips — the body literally letting the air out",
                "eyes closing for a beat too long, the face resetting from hope to acceptance",
                "the smile fading in real time, the corners of the mouth pulling down as the news lands",
                "head tilting back, eyes to the ceiling — the 'why me' posture held for a silent count",
                "hands falling to the sides, palms slapping lightly against the thighs in surrender",
                "collapsing into a chair with none of the usual ceremony of sitting",
                "picking up the thing that failed — the letter, the test, the phone — and setting it down again, gently, as if it were fragile",
                "rubbing the back of the neck, eyes on the ground, processing",
                "the jaw loosening, the mouth opening slightly — not to speak but because the face has simply let go",
                "turning away from the source of the news, the body needing a moment of privacy even in a public space",
                "a half-laugh, bitter and abrupt, the sound of expecting this and getting it anyway",
                "pressing a thumb and forefinger into the corners of the eyes, the headache of recalibration beginning",
                "voice going quiet mid-sentence, the enthusiasm draining out of the words like water from a cracked cup",
                "staring at a spot on the floor, the gaze heavy with the weight of what was imagined versus what is",
            ]),
            ("Internal Sensations", vec![
                "a sinking in the stomach — not the freefall of fear but the slow settling of something heavy finding the bottom",
                "the chemical reversal: the serotonin and dopamine of anticipation crashing, replaced by a flat, gray inertia",
                "a tightness in the throat that is part sadness, part frustrated anger, neither fully forming",
                "the chest deflating, the air leaving in a way that doesn't feel like it will fully return",
                "a prickling behind the eyes — the body preparing to cry over something that doesn't quite merit it",
                "the muscles that were braced for good news going slack all at once",
                "a dull ache in the solar plexus, the place where hope lived moments ago",
                "the sudden, physical tiredness that arrives with the realization that the effort was wasted",
                "warmth draining from the face and hands, the excitement's flush retreating",
                "the strange, hollow hunger of wanting something that was so close and is now simply not",
            ]),
            ("Mental Responses", vec![
                "the rapid recalculation: adjusting the story of what was going to happen to what actually did",
                "the thought arriving with a sting: I should have known better than to hope",
                "replaying the moment of finding out, searching for a misunderstanding that isn't there",
                "the gap between the imagined version and the real one widening like a crack in a wall",
                "a retroactive embarrassment at how excited you were — the enthusiasm now looking naive",
                "the internal accounting: what was invested versus what was received, the ledger dripping red",
                "wondering whether the fault is in the world or in the wanting — was the expectation unreasonable",
                "the future reorganizing itself around the absence of the thing that was expected to be there",
                "a flicker of anger at whoever or whatever failed to deliver, quickly dampened by resignation",
                "the thought 'what now?' sitting in the mind like an unanswered question in an empty room",
            ]),
            ("Suppression Cues", vec![
                "shrugging and saying 'it's fine, it wasn't a big deal' while the voice catches almost imperceptibly",
                "forcing the shoulders back up, the posture corrected before anyone notices the slump",
                "smiling quickly — a flash of teeth that says 'I'm not bothered' — before the face can settle into what it actually feels",
                "pivoting immediately to the next plan, the next option, the next thing — outrunning the feeling with logistics",
                "making a joke at one's own expense: 'well, that's what I get for getting my hopes up'",
                "nodding along as if the outcome was expected all along: 'yeah, I kind of figured'",
                "burying the disappointment in encouragement for someone else's success in the same moment",
                "reaching for the phone, the drink, the next distraction — anything to close the gap between this moment and the next one",
                "exhaling slowly through the nose and straightening the papers, the tie, the expression — tidying up the evidence",
                "filing it under 'not meant to be' with a speed that suggests the philosophy was prepared in advance for exactly this purpose",
            ]),
        ]),
        ("Nostalgia", "Bittersweet longing for a past that is gone, idealized by distance", vec![
            ("Physical Signals", vec![
                "a faraway look settling over the face, the eyes focused on something no one else can see",
                "fingers tracing the edge of an old photograph, the touch tender and slow",
                "lifting a forgotten object — a toy, a ticket stub, a dried flower — and turning it over as if it were a relic",
                "inhaling deeply near something that carries the scent — a jacket, a kitchen, a street after rain — and holding the breath to keep it",
                "the half-smile that arrives unbidden and stays, the expression of someone remembering warmth",
                "humming a song without realizing it, the melody surfacing from a decade-old layer of memory",
                "pausing in a doorway, a hallway, a park — any place where the present overlaps with a version of the past",
                "pressing play on a song and closing the eyes, letting the sound reconstruct an entire world",
                "speaking more slowly when describing that time, the voice softening, the words chosen with care",
                "the hand moving to the chest, resting over the heart, the gesture unconscious",
                "revisiting a place and standing still in it, letting the gap between then and now become visible",
                "laughing at a memory and then going quiet, the laugh trailing into something more complex",
                "pulling a blanket tighter, curling up — the body seeking the warmth that nostalgia promises",
                "running a thumb across handwriting on an old letter, reading the person through their penmanship",
            ]),
            ("Internal Sensations", vec![
                "a warmth spreading through the chest — literal, measurable, the body's reward system firing at a memory",
                "the sting behind the eyes that comes not from sadness but from beauty recalled at a distance",
                "a tightness in the throat — the lump of wanting to be back there and knowing the door is closed",
                "the curious sensation of being in two times at once — the body here, the self somewhere years away",
                "a sweetness suffusing the whole torso, followed immediately by a pang, the two feelings braided together",
                "the skin prickling at a familiar song the way it prickles at a touch — the nerve endings remembering",
                "a hollowness that is not empty but full — full of what was, pressing against the walls of the present",
                "the physical comfort of the memory acting as a blanket, the warmth arriving even in a cold room",
                "the heart rate slowing, the breathing deepening — the parasympathetic calm of returning to safety, even imagined safety",
                "a heaviness in the chest that is not grief but proximity to grief — the sadness standing one room away, visible through the doorway",
            ]),
            ("Mental Responses", vec![
                "the past arriving not as facts but as atmosphere — the quality of the light, the temperature of the air, the sound of a particular door closing",
                "the idealization working quietly in the background, softening edges, warming colors, editing out the bad days",
                "the thought forming with perfect clarity: we didn't know how good it was while we were in it",
                "a sudden, vivid flash of a specific moment — not an important one, just a Tuesday, a meal, a drive — made precious by distance",
                "the awareness that what is being remembered no longer exists in any form except inside the mind that holds it",
                "a desire to tell someone about it, to make them see it, knowing the words will never carry the full weight",
                "the mental time-travel that is nostalgia's signature — the self slipping backward while the body stays anchored",
                "wondering which of today's ordinary moments will become tomorrow's nostalgia, and the ache of not knowing",
                "the realization that missing something is also a way of still having it",
                "the gentle, persistent question: was I happier then, or does distance just make it look that way",
            ]),
            ("Suppression Cues", vec![
                "putting the photograph back in the drawer, the letter back in the box — closing the portal deliberately",
                "shaking the head and laughing it off: 'sorry, I'm being ridiculous'",
                "switching the song, the channel, the subject — cutting the trigger before the feeling deepens",
                "returning to the present with a brisk physical movement — standing up, clapping hands together, checking the time",
                "converting the feeling into an anecdote, performing the memory for others rather than sitting in it alone",
                "dismissing it as sentimentality: 'it probably wasn't as good as I remember'",
                "anchoring to a task — something with a deadline, a purpose — to pull the mind forward out of the past",
                "making a joke about getting old, about being 'such a sap,' about the absurdity of missing what can't be returned to",
                "scrolling forward through the photos rather than lingering, refusing the invitation to stay",
                "telling oneself that looking back is a waste when there's still road ahead — the rational override of an irrational warmth",
            ]),
        ]),
        ("Homesickness", "The physical ache of being far from the place or people that feel like home", vec![
            ("Physical Signals", vec![
                "staring at a phone screen showing a distant time zone, calculating what they're doing right now",
                "crying at unexpected triggers — an accent overhead, a brand of cereal, a particular shade of afternoon light",
                "cooking a recipe from home with obsessive precision, as if the taste could close the distance",
                "sleeping with a familiar object — a shirt, a blanket, something that still carries the scent of the other place",
                "standing at a window facing the direction of home, even when there's nothing to see but the next building",
                "hovering over the call button, doing the math on time zones, then putting the phone away again",
                "wandering the aisles of a grocery store looking for a product they'll never find in this country",
                "speaking in the home language to oneself — muttering while cooking, swearing under the breath, counting in the mother tongue",
                "the body going listless on certain days — weekends, holidays, the times when home was loudest",
                "rearranging a rented room to echo the arrangement of a room left behind",
                "clutching the phone during a video call, leaning close to the screen as if proximity to the image is proximity to the person",
                "pulling up maps of home on a screen, zooming in on a street, a building, a corner — traveling without moving",
                "losing interest in the new surroundings, the eyes sliding past landmarks that once held curiosity",
                "wearing a piece of jewelry, a team scarf, a pin from home — the talisman that says where you belong",
            ]),
            ("Internal Sensations", vec![
                "a yawning hollow in the stomach that food doesn't fill — the body hungry for something that isn't a meal",
                "the chest aching with a dull, persistent pressure, as if homesickness has settled in and furnished the space",
                "a jolt of recognition at a familiar smell — bread baking, rain on hot pavement, a certain soap — followed immediately by the sting of displacement",
                "the stomach knotting on Sundays, on holidays, on any day the calendar says you should be somewhere else",
                "fatigue arriving not from activity but from the effort of being a stranger all day, every day",
                "the throat closing when someone asks 'where are you from?' — the answer too large for the question",
                "a low-grade nausea that sharpens at transitions: waking in an unfamiliar room, hearing the wrong birdsong, missing the sound of a door that isn't here",
                "the body temperature feeling wrong — too cold in summer, too warm in winter — as if the internal thermostat is calibrated to a different climate",
                "a headache that arrives in the late afternoon, the fog of displacement thickening toward evening",
                "the physical weight of being unknown — no one on this street knows your name, your history, your context",
            ]),
            ("Mental Responses", vec![
                "the preoccupying thoughts of home that hijack concentration — a lecture, a meeting, a paragraph all lost to daydreaming of a kitchen, a yard, a voice",
                "idealizing home until it glows — forgetting the reasons for leaving, remembering only what was left behind",
                "the mental calendar counting days: how long since, how long until, the arithmetic of absence",
                "comparing everything — the coffee is wrong, the sky is the wrong blue, the bread doesn't taste like bread",
                "the strange guilt of enjoying something here, as if pleasure in the new place is betrayal of the old",
                "a recurring fantasy of the return — the airport, the drive, the door opening, the faces — rehearsed so many times it has the clarity of memory",
                "wondering if home is changing without you, and whether you'll still fit when you go back",
                "thoughts snagging on small, specific absences: a grandmother's cooking, a friend's laugh, the sound of the evening call to prayer, the particular creak of a staircase",
                "the disorienting awareness that life is continuing in two places at once, and you can only be in one",
                "the question arriving on the hard days: did I make a mistake coming here",
            ]),
            ("Suppression Cues", vec![
                "throwing energy into the new — exploring, socializing, saying yes to everything — outrunning the ache with motion",
                "refusing to call home too often: 'it only makes it worse'",
                "decorating the new space aggressively, making it 'theirs,' as if ownership could replace belonging",
                "dismissing it with a shrug: 'everyone goes through this, it'll pass'",
                "avoiding other expats or immigrants to avoid the collective amplification of the feeling",
                "learning the new language, the new customs, the new routes with a ferocity that looks like enthusiasm but is actually defense",
                "keeping the video calls short and upbeat, performing adjustment for the people who worry",
                "unfollowing social media from home so the feed stops delivering the life happening without you",
                "telling the story of leaving as an adventure rather than an exile — controlling the narrative to control the feeling",
                "filling the apartment with noise — podcasts, music, the television — so the silence never has a chance to remind you how far away you are",
            ]),
        ]),
        // ── Joy & Pleasure Family ──
        ("Joy", "Bright, expansive happiness — the feeling that life is good and the world is generous", vec![
            ("Physical Signals", vec![
                "the Duchenne smile — involuntary, crinkling the corners of the eyes, impossible to fake",
                "laughing freely and fully, the head thrown back, the sound spilling out unchecked",
                "a lightness in the step that borders on bouncing — the body unable to move at a normal pace",
                "arms thrown wide as if trying to hold the whole moment at once",
                "spinning around, the movement spontaneous and unselfconscious as a child's",
                "hugging someone hard enough to lift them, or being lifted, feet leaving the floor",
                "clapping both hands over the mouth, the joy too big for the face to contain",
                "eyes bright and wet — tears arriving not from pain but from the sheer overflow of the feeling",
                "dancing in the kitchen, in the hallway, in the car — the body insisting on movement",
                "speaking faster, voice climbing in pitch, words tumbling over each other",
                "the face glowing — cheeks flushed, skin warm, the whole complexion lit from the inside",
                "punching the air with a fist, the gesture tight and triumphant",
                "squeezing someone's hand, their arm, their shoulder — needing physical contact to ground the feeling",
                "humming without realizing it, the melody leaking out between tasks",
                "standing taller, shoulders open, chin up — the posture of someone who has just been given the world",
                "the grin that won't leave, held for so long the cheeks begin to ache",
            ]),
            ("Internal Sensations", vec![
                "warmth blooming in the chest and radiating outward, as if the heart is a sun with its own light",
                "a fizzing, carbonated energy under the skin — every nerve ending awake and delighted",
                "the heart beating faster but lighter, each pulse a percussion of aliveness",
                "the lungs filling deeper and easier than they have in weeks, the breath suddenly effortless",
                "a lightness in the limbs, the body buoyant, as if gravity has loosened its grip by half",
                "the dopamine flood — a rush of pleasure so physical it borders on dizziness",
                "the stomach fluttering, not with anxiety but with the kinetic thrill of something wonderful happening",
                "a tingling across the scalp and down the spine — the body's standing ovation",
                "the muscles releasing tension they've been holding without awareness, the whole body softening and opening",
                "a sweetness in the throat, the opposite of the lump of sadness — an expansion, a loosening",
                "the sense of time slowing in the best way, each second savored and full",
            ]),
            ("Mental Responses", vec![
                "the world looking sharper, brighter, more saturated — as if someone has adjusted the contrast on reality",
                "an overwhelming urge to share the feeling, to call someone, to tell everyone",
                "the thought arriving with quiet wonder: this is happiness, this is what it feels like, I want to remember this",
                "a generosity of spirit that extends to strangers — holding doors, tipping more, forgiving small annoyances",
                "the sense of invincibility, the feeling that nothing could go wrong today, that the universe is on your side",
                "gratitude arriving alongside the joy, the two emotions so intertwined they're hard to separate",
                "optimism projecting outward — seeing possibility everywhere, imagining futures that glow",
                "thoughts becoming playful, creative, quick — the mind at its most nimble and generous",
                "a willingness to be silly, to sing badly, to dance without skill, to be fully and unguardedly yourself",
                "the awareness that this will pass — and rather than dimming the joy, the awareness deepens it",
            ]),
            ("Suppression Cues", vec![
                "pressing the lips together hard to keep the grin from splitting the face — the muscles fighting the restraint",
                "looking down or away to hide eyes that are visibly shining",
                "clearing the throat and straightening up, trying to project composure when the body wants to leap",
                "converting the joy into controlled enthusiasm: 'that's really great news' in a measured voice that vibrates at the edges",
                "biting the inside of the cheek, the small pain a brake against the smile that wants to take over",
                "fidgeting with something to channel the buzzing energy — a pen, a button, the hem of a sleeve",
                "downplaying the source of joy to avoid seeming boastful: 'oh, it's not a big deal, really'",
                "leaving the room quickly so the celebration can happen in private — the fist pump behind the closed door",
                "restraining the urge to hug someone by squeezing one's own hands together instead",
                "the joy leaking through despite every effort — a brightness in the voice, a quickening of pace, a warmth in the eyes that no amount of composure can fully mask",
            ]),
        ]),
        ("Contentment", "A quiet, settled satisfaction with the present moment — happiness without urgency", vec![
            ("Physical Signals", vec![
                "leaning back in a chair with a long, slow exhale — the satisfied sigh that says the day is done",
                "hands resting open and still in the lap, the fingers uncurled, no fidgeting, no reaching",
                "a soft half-smile that doesn't demand attention — just there, the face at rest in its kindest position",
                "eyes half-lidded, the gaze warm and unfocused, not looking for anything",
                "movements gone unhurried — the cup lifted slowly, the page turned without haste",
                "shoulders dropped low and loose, the posture of a body that has forgotten about its armor",
                "bare feet tucked under a blanket, the small act of self-comfort that signals nothing is required",
                "humming absently while doing something simple — dishes, folding, watering a plant",
                "resting a chin on someone's head, an arm draped around a shoulder — the contact lazy and confident",
                "watching rain through a window with no desire to be anywhere else",
                "stroking the fur of a sleeping cat, the rhythm slow and meditative",
                "a deep breath taken not for calm but for pleasure — savoring the air itself",
                "lingering at the table after the meal is finished, in no rush to clear or leave",
                "stretching luxuriously, the kind of stretch that involves sound — a groan, a sigh, the whole body lengthening",
            ]),
            ("Internal Sensations", vec![
                "a warmth settled deep in the center of the chest, not radiating outward but glowing steadily in place",
                "the muscles so relaxed they feel liquid — the body heavy in the best way, sinking into whatever holds it",
                "breathing slow and deep, the lungs filling to capacity and emptying without effort — the parasympathetic hum of safety",
                "the stomach settled and full, digestion working quietly, the body attending to the ordinary business of being alive",
                "a drowsy, pleasant heaviness in the limbs — not fatigue but the body's signal that no action is required",
                "the heart beating at its slowest waking pace, steady and unhurried, each beat a metronome of calm",
                "the absence of tension so complete it's almost a sensation in itself — the jaw loose, the shoulders low, the forehead smooth",
                "a suffusion of pleasure that has no peak and no urgency — it simply persists, like background warmth",
                "the skin sensitive in a pleasant way — the texture of the blanket, the temperature of the air, small pleasures amplified",
                "the sense that the body and the moment are the same size — nothing too large, nothing missing",
            ]),
            ("Mental Responses", vec![
                "the thought arriving quietly and without fanfare: this is enough",
                "an absence of wanting — the mind not reaching forward or looking back, just here",
                "noticing small things with unusual clarity: the quality of the light, the sound of a clock, the grain of the wood",
                "gratitude rising not as a dramatic emotion but as a background hum — a steady awareness of what is good",
                "the rare, precious cessation of the internal monologue — the mind not planning, not worrying, not rehearsing, just still",
                "thoughts drifting without urgency, following no thread, landing on nothing important and content to stay there",
                "the willingness to let the moment be what it is without improving it, photographing it, or narrating it",
                "a gentle awareness that this feeling is temporary, and choosing not to let that awareness diminish it",
                "the sense that all the necessary pieces are in place — not everything, not perfectly, but enough, and enough is everything",
                "the thought forming in its simplest version: I am okay, right now, exactly as things are",
            ]),
            ("Suppression Cues", vec![
                "feeling guilty for the contentment and disrupting it with productivity — standing up, making a list, finding a task",
                "dismissing the feeling as laziness: 'I should be doing something'",
                "checking the phone, breaking the spell with the world's demands",
                "mentioning the contentment aloud and immediately qualifying it: 'but I know it won't last'",
                "shifting to planning mode — if things are good now, the mind wants to strategize how to keep them that way",
                "looking for what's missing instead of staying with what's present — the habit of vigilance reasserting itself",
                "feeling the contentment and mistrusting it, as if ease is the prelude to something going wrong",
                "telling oneself that happy people don't just sit here — they pursue, they achieve, they move",
                "the faint discomfort of stillness for someone unaccustomed to it — the body wanting to do, the contentment asking to just be",
                "converting the feeling into an Instagram moment rather than living in it — the documentation replacing the experience",
            ]),
        ]),
        ("Euphoria", "Joy amplified to the point of overflow — an almost intoxicating sense of aliveness", vec![
            ("Physical Signals", vec![
                "head tipping back, eyes closing, mouth opening — the face surrendering to the sensation",
                "spinning with arms outstretched, the movement involuntary, the body demanding to express what words can't hold",
                "laughing that escalates until it becomes breathless, tears streaming, ribs aching",
                "grabbing the nearest person and shaking them by the shoulders: 'do you understand what just happened?'",
                "pupils blown wide, the eyes black and glittering, seeing everything as extraordinary",
                "skin flushed and luminous, the cheeks hot, the whole body radiating heat",
                "jumping — not once but repeatedly — the feet leaving the ground as if the body refuses to be earthbound",
                "speaking too fast, words crashing into each other, the mouth unable to keep pace with the mind",
                "hands trembling — not from fear but from the sheer voltage of the feeling running through the nervous system",
                "arms thrust overhead in a V, fists clenched, the primal victory pose",
                "visible gooseflesh racing up the arms despite the warmth, the body responding to its own electricity",
                "the inability to sit, to stand, to stay in one position — the energy demanding motion in every direction",
                "happy tears arriving without permission, the crying indistinguishable from the laughing",
                "kissing someone with sudden, fierce abandon — or hugging a stranger, or shouting at the sky",
                "the voice rising to a shout, a whoop, a sound that has no words in it, just raw exhilaration",
            ]),
            ("Internal Sensations", vec![
                "a dopamine flood so intense the world seems to sharpen and glow, every color brighter, every edge crisper",
                "the heart hammering not with fear but with aliveness — each beat a detonation of pure feeling",
                "warmth erupting from the core and pouring outward, the skin tingling from the inside",
                "a lightness so extreme the body feels weightless, as if the bones have been replaced with helium",
                "the breath coming in gasps, the lungs unable to match the body's demand for more air, more oxygen, more",
                "endorphins and endocannabinoids crashing through the system — the body's own narcotics, the natural high without a ceiling",
                "a buzzing in every nerve ending, the whole body vibrating at a frequency just above normal",
                "the scalp tingling, the fingertips electric, the sensation of every hair on the body standing at attention",
                "a pressure in the chest that isn't pain but fullness — the feeling too large for the container",
                "the strange, blissful numbness to everything negative — cold, hunger, exhaustion, pain — all temporarily erased",
                "time doing something impossible: simultaneously racing and standing still, each second infinite and gone in a flash",
            ]),
            ("Mental Responses", vec![
                "the absolute conviction that anything is possible — a feeling of omnipotence that erases the word 'can't'",
                "thoughts expanding, connecting, leaping — the mind moving at a speed and breadth that feels superhuman",
                "the desire to make enormous decisions, grand gestures, life-changing declarations — impulse unchained from caution",
                "the world reorganized around this moment as the peak, the point, the reason — everything before was leading here",
                "a generosity that borders on recklessness — the urge to give everything away because there's more than enough",
                "the thought arriving with the force of revelation: this is what being alive is for",
                "a total dissolution of self-consciousness — no awareness of being watched, judged, or contained",
                "the awareness fluttering at the very edge of the feeling: this can't last, which only makes the burning brighter",
                "the dangerous clarity of someone who feels invincible — judgment blurred, risk appetite enormous",
                "the desire to freeze the moment, to live inside it forever, the first faint pang of knowing you can't",
            ]),
            ("Suppression Cues", vec![
                "pressing both hands over the mouth, physically capping the scream or the sob of joy",
                "biting down on a knuckle, channeling the unbearable intensity into a single point of pressure",
                "gripping the edge of a table, the railing, anything solid — trying to anchor the body against the lift",
                "closing the eyes and breathing through it, as if the feeling is a wave that must be survived rather than ridden",
                "converting the shout into a tight, trembling whisper: 'oh my god, oh my god'",
                "covering the face with both hands, the expression behind them too raw to share",
                "walking in a tight circle, channeling the energy into motion because standing still would mean exploding",
                "the strangled laugh that is half-cry, the body unable to decide which release valve to use",
                "turning away from people to compose the face, only to turn back still incandescent and unable to hide it",
                "the attempt at composure lasting exactly one breath before the grin breaks through and the dam fails completely",
            ]),
        ]),
        ("Relief", "The release of tension after a threat passes — the body unclenching, followed by ease", vec![
            ("Physical Signals", vec![
                "the long, shuddering exhale — lungs emptying completely, the breath held for minutes finally released",
                "shoulders dropping three inches, the tension pouring out of them like sand from a fist",
                "legs buckling — knees giving way, the body sliding down a wall or sinking into the nearest chair",
                "hands covering the face, the fingers pressing into the eye sockets, the head shaking slowly in disbelief",
                "laughing — sudden, breathy, uncontrollable — the sound of a system releasing pressure",
                "crying that arrives only after the danger has passed, as if the body waited for permission",
                "hands trembling visibly, the fine motor control shot by the adrenaline that has nowhere left to go",
                "leaning forward, palms on knees, head hanging, breathing hard — the posture of someone who just finished sprinting",
                "pulling someone into a fierce, crushing hug — holding on as if letting go might undo the safety",
                "closing the eyes and tipping the head back, the face aimed at the ceiling, the sky, anything above",
                "a hand placed flat over the heart, feeling it hammer, waiting for it to slow",
                "the whole body going loose and boneless, collapsing into a seat as if the skeleton has been removed",
                "wiping the eyes with the back of the hand, the gesture rough and surprised — 'I didn't realize I was crying'",
                "the nervous, giddy laughter that bubbles up between gasps, inappropriate and unstoppable",
                "standing very still for a long moment, just breathing, as if the act of not-being-in-danger needs to be confirmed",
            ]),
            ("Internal Sensations", vec![
                "the knot in the stomach loosening all at once, the tension unraveling like a rope dropped from a great height",
                "a wave of warmth flooding inward — the blood returning to the extremities, the hands and feet tingling back to life",
                "the heart rate dropping from hammering to heavy to normal, each deceleration felt as a distinct downshift",
                "adrenaline crashing — the spike receding and leaving behind a shaky, hollow, wrung-out exhaustion",
                "muscles that were clenched for hours releasing simultaneously, the ache of unclenching almost as intense as the tension itself",
                "a lightness rising through the chest, the weight literally lifting — the phrase 'a weight off my shoulders' is not a metaphor",
                "the sudden, overwhelming urge to sleep — the parasympathetic system pulling the body toward rest with the force of a tide",
                "nausea arriving paradoxically in the moment of safety, the stomach processing the fear it was too activated to feel before",
                "the jaw unclenching, the teeth separating for the first time in what feels like hours, the hinge aching",
                "the full-body shiver that rolls through once, head to feet, as if the nervous system is resetting itself",
                "a sweetness flooding in behind the fear — the world looking impossibly vivid and dear, as if seen for the first time",
            ]),
            ("Mental Responses", vec![
                "the thought arriving on repeat, simple and stunned: it's over, it's over, it's actually over",
                "the mind rewinding to the worst moment and flinching — then pulling back to the present, to the safety, savoring the contrast",
                "a gratitude so intense it borders on religious — thank you, thank you, spoken to no one in particular",
                "the disbelief that the feared outcome didn't happen, the mind still braced for it, slow to accept the reprieve",
                "a sudden, fierce tenderness for everything — the room, the daylight, the sound of ordinary traffic — all of it precious",
                "the realization of how tightly you were wound, visible only now in the unwinding",
                "thoughts loosening, unfocusing, losing their frantic edge — the mind allowing itself to wander for the first time",
                "the retroactive fear: understanding how bad it could have been, now that the danger has cleared enough to think about it",
                "an urge to call someone, to hear a voice, to confirm connection — the social instinct reengaging after the crisis",
                "the thought that tomorrow will exist after all — the future flickering back on like a light that had been switched off",
            ]),
            ("Suppression Cues", vec![
                "straightening up quickly, brushing off the emotion: 'I'm fine, I'm good, we're good'",
                "swallowing the tears and converting the relief into brisk action — what's next, what do we do now",
                "clearing the throat and forcing the voice steady, refusing to let it crack in front of others",
                "making a joke to defuse the vulnerability of the moment: 'well, that was fun'",
                "pressing the trembling hands flat on a surface, stilling them through force of will",
                "wiping the face and assuming a composed expression before turning back to the group",
                "channeling the relief into logistics — calling someone, checking a list, doing the next task — because sitting in the feeling is too exposing",
                "the tight nod and the quiet 'okay' that acknowledges the relief without surrendering to it",
                "offering reassurance to others instead of accepting it — caretaking as a deflection from one's own rawness",
                "laughing it off as 'no big deal' while the hands are still shaking and the heart is still finding its rhythm",
            ]),
        ]),
        ("Satisfaction", "The warm, earned sense of completion — effort met with result", vec![
            ("Physical Signals", vec![
                "leaning back in the chair with hands laced behind the head — the posture of someone surveying finished work",
                "a slow, closed-mouth smile that deepens rather than flashes — the expression of someone savoring rather than celebrating",
                "brushing the hands together once — the universal gesture that says 'that's done'",
                "a deep breath drawn in through the nose and released slowly, deliberately, the exhale carrying tension out with it",
                "setting down a tool, a pen, a phone with a quiet finality — the object placed as if the last period in a sentence",
                "eyes moving over the completed thing — the painting, the document, the room — the gaze appraising and pleased",
                "the small nod to oneself, barely perceptible, the private acknowledgment that it is good",
                "stretching the arms overhead, the body elongating, celebrating the end of effort with the luxury of ease",
                "pouring a drink, making a meal, running a bath — the reward ritual that follows the work",
                "standing back from the finished thing with arms folded, not defensively but with quiet possession",
                "the pace slowing, movements becoming unhurried — the body downshifting from work mode to rest",
                "wiping the hands on a cloth, an apron, the thighs — the symbolic cleaning of effort's residue",
                "offering the result to someone else with a casual modesty that doesn't quite hide the pride underneath",
            ]),
            ("Internal Sensations", vec![
                "a warm glow in the center of the chest — not the blaze of joy but a steady, banked heat",
                "the dopamine arriving precisely, cleanly — the brain's 'mission accomplished' signal, specific and earned",
                "muscles releasing their task-tension gradually, the body acknowledging it can stop now",
                "a pleasant heaviness settling into the limbs — the good fatigue that follows real effort",
                "the stomach unclenching, appetite returning — the body ready to accept nourishment now that the work is done",
                "a fullness behind the ribs that isn't pressure but presence — the feeling of something completed taking up space where the task used to be",
                "the hands tingling faintly, the fingers aware of what they just built, wrote, fixed, finished",
                "breathing slowing to its deepest, most even rhythm — the body's metronome resetting to calm",
                "a quiet hum of wellbeing that doesn't demand expression, just persists like background music turned low",
                "the particular pleasure of tired muscles — the ache that proves something was done, that the effort was physical and real",
            ]),
            ("Mental Responses", vec![
                "the thought arriving with simplicity and weight: that was worth it",
                "mentally tracing the arc from start to finish — remembering the difficulty and measuring it against the result",
                "the rare cessation of the internal critic — the voice that says 'not good enough' briefly, blessedly quiet",
                "a willingness to be finished, to stop refining, to accept that done is its own kind of perfect",
                "looking at the next task and feeling, for this moment, no urgency about it — this one is complete, and that's enough",
                "the quiet pride of craftsmanship — not showy, not boastful, just the knowledge that care was taken and it shows",
                "gratitude toward the self that persisted — a gentleness directed inward for once",
                "the desire to sit with the accomplishment before moving on, to let it register before the next thing demands attention",
                "comparing the finished thing to the imagined version and finding the gap small — or closed entirely",
                "the thought that tomorrow's effort will be easier because today's was completed — momentum building, quietly, in the background",
            ]),
            ("Suppression Cues", vec![
                "downplaying the accomplishment immediately: 'it's nothing, really' — the deflection arriving before the compliment can fully land",
                "moving on to the next task without pausing, as if lingering in satisfaction is self-indulgent",
                "crediting the outcome to luck, to help, to circumstances — anything but the effort itself",
                "shrugging off praise with a quick subject change: 'anyway, what's next?'",
                "the smile suppressed into a tight line, the pleasure contained behind a mask of professionalism",
                "finding a flaw in the finished work to preempt anyone else finding it first",
                "offering the satisfaction to others instead: 'we did this' rather than sitting with 'I did this'",
                "cleaning up immediately, putting the tools away, erasing the evidence of effort as if the result appeared by magic",
                "the discomfort of being seen in a moment of earned pleasure — crossing arms, looking away, busying the hands",
                "qualifying the satisfaction: 'it's good, but next time I'd change...' — the amendment arriving before the period",
            ]),
        ]),
        ("Pride", "The self-affirming pleasure of having met your own standard", vec![
            ("Physical Signals", vec![
                "standing taller — the spine straightening, the shoulders pulling back, the body claiming its full height",
                "chin lifting slightly, the head tilted just enough to look out at the world rather than up at it",
                "chest expanding, the ribcage opening — the universal pride posture that even blind-from-birth athletes perform",
                "hands settling on the hips, elbows out — the expansive stance of someone who has earned the space they occupy",
                "a smile that starts small and grows, the expression confident and unhurried",
                "making eye contact easily, directly, holding it without challenge — the gaze of someone with nothing to prove",
                "walking with a visible spring, the stride longer, the movement carrying a quiet authority",
                "gesturing more broadly while speaking — the hands opening outward, the movements expansive",
                "touching the finished thing — the diploma, the building, the child's shoulder — the physical connection to what was accomplished",
                "the controlled nod when receiving praise — accepting it without deflecting, without rushing past it",
                "straightening a tie, adjusting a collar, smoothing a lapel — the grooming gestures of someone presenting their best self",
                "arms raised overhead in a V, the fists clenched — the victory display hardwired into the species",
                "the gleam in the eye that accompanies the telling of the story — 'let me show you what we did'",
                "clapping someone on the back whose success you helped build — pride expressed through another's achievement",
            ]),
            ("Internal Sensations", vec![
                "a swelling in the chest — not pressure but expansion, the ribcage widening to accommodate the feeling",
                "warmth spreading upward from the sternum to the throat, the face, the ears — the flush of self-recognition",
                "the spine tingling with a current of energy that straightens the posture from the inside",
                "a buoyancy in the limbs, the body lighter, as if the accomplishment has reduced its own gravity",
                "the heart beating with a steady, confident rhythm — not the hammering of excitement but the strong pulse of assurance",
                "the muscles of the face pulling into a smile involuntarily, the expression arriving before the conscious decision to make it",
                "a quiet heat behind the eyes — not tears but the intensity of self-acknowledgment, the emotion of meeting one's own gaze in the mirror and approving",
                "the hands feeling capable, strong, aware of what they've done — a physical memory of the effort living in the fingers",
                "a groundedness in the feet, the stance wide and solid — the body announcing that it belongs exactly here",
                "the dopamine arriving not as a flood but as a glow — the reward system confirming what the self already knew",
            ]),
            ("Mental Responses", vec![
                "the thought arriving with a clarity that cuts through imposter syndrome: I did this, and it is good",
                "mentally retracing the path — the early doubt, the middle difficulty, the late persistence — and finding the arc worthy",
                "the temporary suspension of self-criticism, the inner voice going quiet or, for once, kind",
                "a desire to be seen, not for vanity but for validation — the work happened, and it matters that someone knows",
                "the awareness of having grown — being more than what you were before this effort, this risk, this choice",
                "pride extending outward to others who helped: the team, the mentor, the person who believed before you did",
                "the sense of standard met — not perfection but the particular bar that was set, cleared, and acknowledged",
                "a flicker of protectiveness over the accomplishment — the instinct to defend it against diminishment",
                "looking at the next challenge with a confidence borrowed from this success: if I did that, I can do this",
                "the rare, nourishing thought: I am enough, right now, as proven by what I have done",
            ]),
            ("Suppression Cues", vec![
                "deflecting praise immediately: 'oh, it was a team effort' — giving the credit away before it can be fully received",
                "making the posture smaller on purpose — shoulders rounding, chin dropping — to avoid appearing boastful",
                "the smile pressed into a line, the pride swallowed to stay humble",
                "changing the subject quickly after a compliment, uncomfortable with the spotlight",
                "attributing the success to luck, timing, or circumstances — anything except one's own capability",
                "minimizing the achievement: 'it's really not a big deal, anyone could have done it'",
                "looking down or away when praised, the body physically retreating from the acknowledgment",
                "the cultural or familial conditioning surfacing: 'don't get a big head' echoing in the internal monologue",
                "focusing on what still needs to be done rather than what was accomplished — the deflection disguised as diligence",
                "accepting the pride privately — the small smile in the car alone, the fist pump behind the closed door — while showing nothing in public",
            ]),
        ]),
        ("Amusement", "The light, pleasant surprise of something funny or absurd — the forerunner of laughter", vec![
            ("Physical Signals", vec![
                "the corners of the mouth twitching upward before the brain gives permission",
                "a quick, bright laugh — the sound escaping like air from a punctured balloon",
                "eyes crinkling, the crow's feet deepening, the whole face reorganizing around the smile",
                "one eyebrow lifting, the expression of someone who has caught the joke before it's finished",
                "the head tilting back slightly, a chuckle rising from the chest",
                "nudging the nearest person with an elbow — the instinct to share the moment, to confirm someone else sees it too",
                "covering the mouth with a hand, the laugh leaking through the fingers anyway",
                "snorting — the ungraceful, involuntary sound that is funnier than the joke itself",
                "eyes darting to a co-conspirator across the room, the shared glance that says 'are you seeing this?'",
                "shoulders shaking silently, the laugh held in the body rather than released through the mouth",
                "biting the lip and looking away, trying and failing to maintain composure",
                "the slow grin spreading like ink in water, the amusement building visibly before the laugh arrives",
                "slapping a hand on the table, the knee, one's own thigh — the body needing a percussive outlet",
                "doubling over, hands on knees, the laughter escalating past the ability to stay upright",
                "wiping tears from the corners of the eyes, the face aching from sustained grinning",
            ]),
            ("Internal Sensations", vec![
                "a bubble of warmth rising in the chest, light and pressurized, demanding release",
                "the diaphragm contracting in quick, involuntary spasms — the body's laugh mechanism firing before the mind is ready",
                "endorphins releasing in a quick, bright burst — the brain's reward for solving the incongruity",
                "a fizzing lightness behind the ribs, the sensation of delight taking up physical space",
                "the face muscles pulling into a grin with a force that feels almost hydraulic — automatic and irresistible",
                "the stomach clenching from sustained laughter, the abs working as hard as a sprint",
                "a warmth flooding the face, the cheeks flushing from the exertion and the pleasure",
                "the chest loosening, the breath coming easier — laughter opening what tension had closed",
                "a tingling in the sinuses from a laugh that nearly became a snort",
                "the pleasant, wrung-out ache of muscles that have been laughing too long — ribs sore, jaw tired, eyes wet",
            ]),
            ("Mental Responses", vec![
                "the split-second recognition of the incongruity — the brain expecting one thing and getting another, and liking it",
                "the urge to repeat the thing that was funny, to tell it to someone who wasn't there, to relive the spark",
                "a sudden, buoyant affection for whoever caused the laughter — humor as social glue, bonding in real time",
                "the thought arriving not in words but as a sensation: this is absurd and I love it",
                "the mind replaying the moment and finding it funnier on the second pass",
                "a temporary suspension of worry, criticism, and self-consciousness — the mind on holiday",
                "the awareness that nothing about this is important, and that's precisely what makes it valuable",
                "the delight of being surprised — the joke working because the punchline wasn't where the mind expected it",
                "a generosity of attention — suddenly finding everything slightly funnier, the amusement lowering the threshold",
                "the rare, delicious thought: I needed that",
            ]),
            ("Suppression Cues", vec![
                "pressing the lips together hard, the mouth a sealed line vibrating with contained laughter",
                "biting the inside of the cheek, the pain a desperate brake against the grin",
                "looking down at the table, at the floor, anywhere except at the person — because eye contact will break the dam",
                "coughing to disguise the laugh, the sound fooling no one",
                "the shoulders shaking with the effort of silence, the body betraying what the face is trying to hide",
                "pinching the bridge of the nose, eyes squeezed shut, the whole face clenched against the eruption",
                "taking a long drink of water to give the mouth something to do besides laugh",
                "turning away and pretending to look at something on the phone while the grin is brought under control",
                "the strangled sound that escapes anyway — half-cough, half-snort — louder and more conspicuous than the laugh would have been",
                "composing the face into seriousness for exactly two seconds before the corners of the mouth betray the effort and the whole thing collapses",
            ]),
        ]),
        ("Excitement", "Joy with a forward lean — the electric anticipation of something good about to happen", vec![
            ("Physical Signals", vec![
                "bouncing on the balls of the feet, the body unable to commit to standing still",
                "leaning forward — toward the door, the screen, the stage, whatever is coming — as if pulled by a wire",
                "speaking faster and louder without realizing it, words accelerating with each sentence",
                "eyes wide and bright, pupils dilated, the gaze locked on the source with unblinking intensity",
                "hands that won't stay still — drumming, clapping, wringing, gripping someone's arm",
                "the involuntary gasp or squeal, the sound escaping before the composure can catch it",
                "pacing a small circuit — to the window, back to the phone, to the window again",
                "checking the time obsessively — the clock, the phone, the clock — each glance urging it forward",
                "grinning so wide the face aches, the expression fixed and radiating",
                "sitting on the very edge of a chair, the body ready to launch at the slightest cue",
                "bouncing a knee in rapid-fire rhythm, the energy demanding an outlet the body can't quite provide",
                "grabbing someone by both arms and shaking them: 'this is really happening'",
                "the voice going high and tight, cracking on certain words from the compressed breath",
                "clapping hands together in a single sharp crack — the body percussing its own anticipation",
                "walking too fast, outpacing companions, arriving early, being the first one there",
            ]),
            ("Internal Sensations", vec![
                "adrenaline and dopamine flooding the system simultaneously — the body revved and rewarded at once",
                "the heart hammering not from fear but from the engine of anticipation running at full throttle",
                "a buzzing under the skin, electric and persistent, as if the nervous system has been plugged into a current",
                "the stomach fluttering — butterflies, but the good kind, the ones with iridescent wings",
                "breath coming short and shallow, the lungs too excited to fill properly",
                "warmth spreading through the chest and face, the skin flushing from the inside",
                "pupils widening, the world going brighter and sharper — more detail, more color, more of everything",
                "the muscles coiled and ready, the body primed to move in any direction at speed",
                "a tightness in the chest that is not anxiety but its twin — the same physiology, different label",
                "the strange, pleasant vertigo of standing at the edge of something about to happen",
                "time distorting — the minutes before stretching interminably while the mind races ahead to the moment itself",
            ]),
            ("Mental Responses", vec![
                "the mind projecting forward, rehearsing the moment, fast-forwarding through the waiting to the thing itself",
                "an inability to focus on anything else — conversations half-heard, tasks half-done, the present tense a waiting room",
                "the mental countdown running constantly, each decrement sharpening the anticipation",
                "thoughts coming rapid-fire: what to wear, what to say, what to bring, what if — the planning escalating to frenzy",
                "the superstitious fear of jinxing it — the excitement so intense it starts to feel fragile",
                "imagining the best-case scenario in cinematic detail, then rewinding and playing it again with variations",
                "the urge to tell everyone — to announce, to share, to recruit others into the anticipation",
                "a giddiness that makes serious thinking temporarily impossible, the mind giggling at its own seriousness",
                "the awareness that this anticipation might be better than the event itself — and not caring",
                "the thought arriving with uncomplicated force: I can't wait, I just can't wait",
            ]),
            ("Suppression Cues", vec![
                "pressing the lips together and nodding slowly, performing calm while the eyes give everything away",
                "sitting on the hands to keep them from drumming, fidgeting, or grabbing someone",
                "lowering the voice deliberately, forcing each word to arrive at a measured pace",
                "crossing the legs to stop the knee from bouncing, the remaining energy rerouting to a tapping foot",
                "taking a long, controlled breath and releasing it slowly — the exhale a pressure valve for the building charge",
                "adopting a casual lean against a wall, the posture contradicted by the grin that won't flatten",
                "responding to 'are you excited?' with a studied shrug: 'yeah, should be fun' — the understatement vibrating",
                "channeling the energy into preparation — packing, cleaning, organizing — productive fidgeting disguised as readiness",
                "scrolling the phone without reading, the hands needing motion while the mind counts down",
                "the contained excitement leaking through anyway: the voice half an octave higher, the steps half a beat faster, the smile arriving one second sooner than it should",
            ]),
        ]),
        // ── Awe & Wonder Family ──
        ("Awe", "The overwhelming feeling of encountering something vast that temporarily dissolves the self", vec![
            ("Physical Signals", vec![
                "all movement ceasing — the body going absolutely still, as if held in place by what it's seeing",
                "the mouth falling open, lips parted, jaw softening — not a smile but the face's surrender to the incomprehensible",
                "eyes widening, inner eyebrows lifting — the unique facial expression of awe, distinct from every other positive emotion",
                "the neck tipping back slowly, the head tilting to take in the full scale of whatever towers above",
                "gooseflesh racing up the arms, the back of the neck, the scalp — the ancient mammalian response to the vast",
                "a hand rising to the chest, resting there without intention, as if confirming the self still exists",
                "tears arriving without sadness — the eyes filling from sheer overwhelm, the emotion spilling because it won't fit inside",
                "the breath catching, then releasing in one long, slow exhale — deeper than any conscious breath could be",
                "reaching out to touch the thing — the canyon wall, the painting, the bark of the ancient tree — needing physical proof",
                "standing rooted, feet planted, while everyone else moves on — unable or unwilling to break the spell",
                "whispering 'oh' or 'my god' or nothing at all — the voice dropping to its lowest register or disappearing entirely",
                "the shoulders dropping, all tension leaving at once — the body disarming itself in the presence of something greater",
                "photographs being taken compulsively, then the phone put away because the screen can't hold what the eyes are seeing",
                "turning in a slow circle to take in the full scope — the cathedral ceiling, the star field, the horizon line",
            ]),
            ("Internal Sensations", vec![
                "the vagus nerve firing — heart rate slowing, breathing deepening, the body shifting into its calmest, most open state",
                "chills traveling up the spine and across the scalp in slow waves, the skin responding to something the mind can't process",
                "a tightness in the throat that is not grief but fullness — the body's container feeling briefly, beautifully inadequate",
                "the chest expanding as if the lungs are trying to take in not just air but the entire scene",
                "a tingling warmth spreading from the center outward — oxytocin and dopamine released simultaneously, a cocktail unique to awe",
                "the sensation of physical smallness — the body shrinking in its own perception, becoming a point in a vast field",
                "eyes stinging from not blinking, the gaze locked and unwilling to break, the eyelids forgotten",
                "the stomach dropping slightly — not from fear but from the vertiginous sense of scale, the recognition of immensity",
                "inflammation quieting in the blood — awe is the only positive emotion proven to reduce the body's stress markers",
                "a stillness in the chest that feels almost sacred — the heart beating slower and more deliberately, each pulse a witness",
            ]),
            ("Mental Responses", vec![
                "the self dissolving at the edges — the usual boundaries between 'me' and 'everything else' going translucent",
                "existing mental frameworks crumbling, the mind unable to fit what it's seeing into any known category",
                "the thought arriving not as words but as a full-body understanding: I am very small, and this is very large, and that is okay",
                "time stopping — or rather, time becoming irrelevant, the moment expanding to contain its own eternity",
                "day-to-day concerns evaporating, the to-do list, the argument, the worry — all of it suddenly ridiculous in scale",
                "a hunger to understand paired with the acceptance that understanding may not be possible — and finding peace in that",
                "the urge to share it with someone specific — the person who would feel this the way you do",
                "a sudden, fierce humility — the ego not wounded but willingly set aside",
                "the awareness that this will change something, that you will walk away different than you arrived",
                "the thought that what you're seeing has existed long before you and will exist long after — and finding that comforting rather than terrifying",
            ]),
            ("Suppression Cues", vec![
                "blinking deliberately and looking away, as if the intensity needs to be taken in doses",
                "reaching for the camera or phone — converting the raw experience into an act of documentation to create distance",
                "narrating what you're seeing aloud, the words a scaffold to keep the mind from freefall",
                "making a joke to deflate the enormity: 'well, that's something' — irony as a pressure valve",
                "moving on before the emotion fully lands, walking to the next exhibit, the next viewpoint, staying in motion",
                "swallowing hard, clearing the throat — managing the physical evidence of being moved",
                "attributing the feeling to exhaustion or altitude rather than admitting the thing simply broke you open",
                "turning to facts and information — reading the plaque, googling the height, anchoring in the measurable",
                "saying 'you have to see this' instead of describing what it felt like — outsourcing the emotion to the future",
                "resuming conversation at a normal volume, deliberately rejoining the human-scaled world before the vastness can settle in permanently",
            ]),
        ]),
        ("Wonder", "Wide-open curiosity and delight sparked by something extraordinary and beautiful", vec![
            ("Physical Signals", vec![
                "leaning forward, drawn toward the thing — the body's approach instinct overriding caution",
                "eyes widening, eyebrows lifting, the face opening like a window thrown up on the first warm day",
                "reaching out to touch, to hold, to turn the object over — the hands leading the investigation",
                "head tilting to one side, the posture of a question being asked without words",
                "mouth forming a small 'o' that slowly transitions into a smile — surprise becoming delight",
                "kneeling or crouching to get closer, the body lowering itself to the level of the discovery",
                "turning something over in the hands slowly, the fingers exploring every surface and edge",
                "the eyes darting between details — this part, then this part, then back — the gaze unable to take it all in fast enough",
                "pointing at something with one hand while tugging someone's sleeve with the other: 'look, look at this'",
                "laughing softly — the spontaneous, delighted sound of the brain encountering something it didn't expect",
                "pressing face close to glass, to the surface, to the edge — narrowing the distance between the self and the extraordinary",
                "spinning around to see if someone else noticed, the discovery too good to hold alone",
                "the hands coming together at the mouth, the fingertips touching — the prayer posture of someone enchanted",
                "stepping closer, then closer again — each step an act of curiosity, the opposite of awe's rooted stillness",
            ]),
            ("Internal Sensations", vec![
                "a brightening behind the eyes, as if someone has turned up a dimmer switch inside the skull",
                "the chest lifting and opening — not the expansion of pride but the receptivity of a child presented with a gift",
                "a tingling curiosity that starts in the fingertips and works inward, the body wanting to participate in what the mind is discovering",
                "the heart quickening gently — not the hammering of excitement but a lighter, more playful acceleration",
                "warmth spreading through the face, the flush of delight without self-consciousness",
                "the stomach fluttering with a pleasure that is entirely about the external — about *this thing* rather than about the self",
                "a lightness in the head, almost giddiness, the brain flooded with the reward of novelty",
                "the skin prickling faintly, the body's quiet register of the extraordinary",
                "breath coming quicker and shallower — not from anxiety but from the lungs matching the mind's quickened pace",
                "the sensation of time slowing not because the moment is vast but because the details are so rich they demand attention",
            ]),
            ("Mental Responses", vec![
                "the question arriving before any other thought: how does this work, what is this, what happens if—",
                "the mind opening like a fist unclenching, releasing assumptions, making room for what it doesn't yet understand",
                "a delight in not knowing — the gap between ignorance and understanding experienced as a pleasure rather than a threat",
                "thoughts connecting at unusual angles, the creative mind activated by novelty",
                "the childlike impulse to ask 'why' and 'what if' without caring whether the questions sound naive",
                "mental categories blurring: is this beautiful or strange or impossible or all three at once",
                "the urge to learn more — to read about it, to come back tomorrow, to understand the mechanism behind the magic",
                "a temporary suspension of cynicism, the protective layer of world-weariness dissolved by genuine surprise",
                "the thought arriving with uncomplicated joy: the world is more interesting than I realized",
                "the awareness that this feeling — this specific, alert, delighted curiosity — is what it felt like to be young",
            ]),
            ("Suppression Cues", vec![
                "pulling the hand back before touching, remembering that adults don't reach for things like children do",
                "replacing the wide-eyed expression with a knowing nod — performing familiarity instead of admitting astonishment",
                "asking a technical question instead of expressing the delight — retreating into expertise",
                "saying 'that's interesting' in a measured tone when the internal response is closer to 'that's incredible'",
                "reading the informational plaque with studied concentration rather than standing there grinning",
                "photographing instead of looking — documentation as a socially acceptable substitute for staring open-mouthed",
                "turning the wonder into an anecdote for later rather than inhabiting it now: 'wait till I tell—'",
                "glancing around to see if anyone else is as affected before allowing the reaction to show",
                "the quick self-correction: getting excited, then dialing it back, afraid of seeming unsophisticated",
                "nodding along as if this is all perfectly expected, while the eyes betray everything the composure is trying to hide",
            ]),
        ]),
        ("Admiration", "The warm, upward pull of recognizing excellence or virtue in someone else", vec![
            ("Physical Signals", vec![
                "straightening in the seat, the posture lifting unconsciously in the presence of someone impressive",
                "eyes fixed and bright, the gaze following the admired person with unbroken attention",
                "nodding slowly while listening — not agreement but recognition, the body acknowledging something worthy",
                "leaning forward, elbows on knees, chin resting on folded hands — the posture of rapt absorption",
                "applauding — not the polite, rhythmic kind but the spontaneous burst, hands coming together hard and fast",
                "a smile that holds warmth rather than amusement — soft eyes, closed mouth, the face lit from within",
                "touching the chest lightly with one hand, the unconscious gesture that accompanies feeling moved",
                "turning to the person beside you with raised eyebrows: 'did you see that?'",
                "standing when the person enters — the rise from the chair that is instinct rather than etiquette",
                "the handshake held a beat too long, the grip communicating more than the words",
                "eyes glistening — not quite tears but the brightness that precedes them, the body's quiet salute to excellence",
                "mirroring the admired person's posture without awareness — sitting the way they sit, adopting their gestures",
                "the intake of breath that accompanies witnessing something extraordinary — a gasp that isn't shock but tribute",
            ]),
            ("Internal Sensations", vec![
                "a warmth dilating in the chest — the specific, well-documented sensation of 'opening' that Haidt calls elevation",
                "chills traveling up the arms and across the scalp — admiration is the only other-praising emotion that consistently triggers goosebumps",
                "the heart rate lifting gently, energized rather than alarmed — the body's readiness to *do* something with the feeling",
                "a lump forming in the throat — the physical precursor to tears that come from witnessing something beautiful in a person",
                "a tingling in the hands and limbs, the body filling with a restless, purposeful energy",
                "the chest expanding as if making room for the feeling, the breath deepening involuntarily",
                "a flush of warmth in the face that is neither embarrassment nor exertion but the body's response to recognizing greatness",
                "oxytocin releasing quietly — the bonding hormone activated by witnessing human excellence, drawing you toward connection",
                "an upward pull behind the sternum, as if the emotion is physically lifting something inside",
                "the sensation of one's own smallness experienced not as shame but as inspiration — the gap between here and there perceived as a distance that could be traveled",
            ]),
            ("Mental Responses", vec![
                "the thought arriving with the force of clarity: that is what it looks like when someone is excellent at what they do",
                "mentally tracing the discipline, the years, the sacrifice that must lie behind what looks effortless",
                "the aspiration igniting — I want to be that, do that, reach that, the desire not envious but energized",
                "studying the person with a focus that is neither competitive nor resentful but purely appreciative",
                "the urge to tell others about this person — to boost their reputation, to share the discovery of their brilliance",
                "a willingness to be the student, the apprentice, the one who watches and learns rather than leads",
                "self-evaluation triggered not by comparison but by possibility: if they can do that, what could I become",
                "the grateful awareness that excellence exists — that someone bothered to get this good at this thing",
                "a temporary suspension of cynicism about human potential, the admired person serving as living proof",
                "the thought settling in with quiet conviction: I am better for having seen this",
            ]),
            ("Suppression Cues", vec![
                "converting the admiration into a casual compliment that undersells the feeling: 'yeah, they're pretty good'",
                "looking away before the emotion can reach the eyes, busying the hands with something mundane",
                "framing the praise as analysis rather than feeling: 'technically, what they did was—' keeping it cerebral",
                "mentioning a flaw alongside the praise, balancing the admiration with criticism to avoid seeming starstruck",
                "clapping at the same pace as everyone else, tamping the response to the group's temperature",
                "swallowing the lump in the throat and clearing it with a cough — refusing to be visibly moved by another person",
                "channeling the feeling into competition rather than appreciation — 'I could do that' deployed as armor against the vulnerability of looking up",
                "dismissing the reaction privately: 'don't be ridiculous, they're just a person'",
                "waiting to express the admiration until later, in private, where it can't be mistaken for sycophancy",
                "the tight nod that communicates respect without surrendering the composure — the restrained version of the standing ovation the body wanted to give",
            ]),
        ]),
        ("Aesthetic Appreciation", "The particular pleasure of beauty for its own sake — in music, landscape, or form", vec![
            ("Physical Signals", vec![
                "standing motionless before a painting, the body gone still, the museum crowd flowing past unnoticed",
                "head tilting slightly, the eyes narrowing then softening — the micro-adjustments of someone truly seeing",
                "closing the eyes during a passage of music, the face going smooth and unguarded",
                "the hand rising involuntarily to the mouth or the throat — the gesture of being touched by something intangible",
                "leaning closer to see the brushstroke, the texture, the place where the artist's hand was",
                "a slow, audible exhale — not a sigh but a release, the breath responding to beauty before the mind can name it",
                "eyes filling without warning — not from sadness but from the particular overwhelm of encountering something perfect",
                "turning back for a second look, a third — the feet unwilling to move on when the eyes aren't finished",
                "gooseflesh rising during a crescendo, a key change, a line of poetry — frisson, the skin's tribute to beauty",
                "the jaw loosening, the mouth softening into a parted, receptive expression",
                "running a finger along a surface — the edge of a sculpture, the spine of a book, the grain of aged wood — reading beauty through touch",
                "sitting in silence after the last note fades, unwilling to break the resonance with speech",
                "stepping back to see the whole, then forward to see the detail — the body calibrating its distance to the beautiful thing",
            ]),
            ("Internal Sensations", vec![
                "frisson — the aesthetic chill traveling up the spine and across the scalp, the skin tingling with a pleasure that has no survival purpose",
                "a tightness in the chest that is not pain but the body's acknowledgment of something it cannot contain",
                "the reward circuitry firing — orbitofrontal cortex and striatum activating together, the brain treating beauty the way it treats food, sex, and love",
                "warmth pooling behind the sternum, the same region that lights up for compassion and connection",
                "the breath catching and then deepening, the lungs responding to beauty as if it were oxygen",
                "a prickling moisture at the corners of the eyes — the tears that Stendhal knew, that come from encountering too much beauty at once",
                "the sensation of time suspending — not stopping but becoming irrelevant, the moment expanding to accommodate what is being experienced",
                "a pleasant vertigo, the Stendhal response — the body briefly overwhelmed by the beauty's intensity, the heart racing, the skin flushing",
                "the full-body hum of resonance, as if the beauty has struck a frequency the body was already tuned to",
                "a quieting of the internal noise — the mental chatter dimming, replaced by pure, attentive receptivity",
            ]),
            ("Mental Responses", vec![
                "the thought dissolving into sensation — the analytical mind stepping aside, the experience bypassing language",
                "a fierce desire to understand how it was made — the reverence for craft arriving alongside the pleasure",
                "the awareness that this beauty existed before you saw it and will exist after you leave — and finding that both humbling and reassuring",
                "searching for the word and not finding it — the gap between what is felt and what can be said",
                "the thought arriving not as words but as a certainty: this is what humans can do when they do their best",
                "a desire to return — to come back when the gallery is empty, when the concert repeats, when the light falls this way again",
                "the sense that something has shifted, that you will remember this and it will inform how you see other things",
                "a temporary loss of self-consciousness — the ego quiet, the attention fully given to something outside the self",
                "noticing details that deepen the pleasure: the shadow beneath a hand, the silence between notes, the way the light falls on one particular surface",
                "the bittersweet awareness that beauty of this order is rare — and that recognizing it may be the closest thing to grace",
            ]),
            ("Suppression Cues", vec![
                "moving on at the group's pace when the body wants to stay — the private mourning of beauty left too soon",
                "nodding appreciatively rather than admitting the throat has tightened and the eyes are stinging",
                "defaulting to critique — identifying a technique, naming a style — to avoid the rawness of simply being moved",
                "photographing the painting, the sunset, the building — capturing it because standing there feeling it is too exposed",
                "saying 'that was nice' when the internal experience was closer to being unmade and reassembled",
                "waiting until alone to play the song again, to look at the image again — the private replay kept separate from the public self",
                "reading the program notes, the artist's statement, the label on the wall — anchoring in information to steady the emotional tilt",
                "wiping the eyes quickly and blaming the light, the dust, the allergies — the standard excuses for being publicly moved by beauty",
                "keeping the pace brisk in the gallery, giving each piece a counted measure of attention rather than the unmetered gaze the feeling demands",
                "converting the experience into recommendation — 'you should see this' — sharing the beauty at one remove to avoid the vulnerability of having been broken open by it",
            ]),
        ]),
        ("Entrancement", "Spell-like absorption in something beautiful or strange — time stops, self dissolves", vec![
            ("Physical Signals", vec![
                "the body gone perfectly still, every voluntary movement suspended — only the breath continuing, slow and shallow",
                "eyes unblinking, the gaze locked on the source with a fixity that would look alarming from the outside",
                "the mouth slightly open, the face slack and undefended — all social performance fallen away",
                "a hand frozen mid-gesture, the arm still raised, the person having forgotten they were moving",
                "leaning forward incrementally, unconsciously closing the distance, drawn in by an invisible thread",
                "the head tilting to one side, the posture of complete receptivity — the body a satellite dish aimed at the signal",
                "not responding when spoken to — the name called once, twice, a touch on the shoulder needed to break through",
                "pupils dilated wide, the eyes dark and depthless, drinking in the source of the spell",
                "food going cold on the plate, the drink untouched, the world outside the absorption becoming irrelevant",
                "breathing syncing to the rhythm of whatever holds the attention — the music, the voice, the flickering of firelight",
                "the pen halted mid-word, the book held open at the same page for twenty minutes, the scroll position unchanged",
                "a faint smile arriving without awareness, the face responding to the enchantment with no input from the conscious mind",
                "emerging startled from the trance — blinking, straightening, looking around as if the room has reassembled itself",
            ]),
            ("Internal Sensations", vec![
                "the self-awareness dimming like a turned-down lamp — the dorsolateral prefrontal cortex quieting, the inner narrator going silent",
                "time dissolving — not slowing or speeding but simply ceasing to be measured, hours passing as minutes",
                "the neurochemical cocktail of deep absorption: dopamine sharpening focus, norepinephrine heightening alertness, anandamide softening fear and widening thought",
                "the body's needs receding — hunger, thirst, discomfort, the full bladder — all deprioritized, all forgotten",
                "a warmth suffusing the chest, the reward system activated by pure engagement without goal or effort",
                "the sensation of boundaries dissolving — the edge between self and experience becoming permeable",
                "a tingling stillness in the limbs, the body so deeply at rest it barely registers as present",
                "the breath moving at its own pace, untethered from conscious control, sometimes deepening, sometimes nearly stopping",
                "an afterglow when the spell breaks — serotonin flooding in, the satisfying exhaustion of a mind that gave itself completely",
                "the disorientation of return: the room too bright, the chair suddenly uncomfortable, the body remembering its own weight",
            ]),
            ("Mental Responses", vec![
                "the complete cessation of the inner monologue — no commentary, no judgment, no planning, just reception",
                "awareness narrowing to a single channel until nothing else exists: not the room, not the day, not the self",
                "the experience bypassing analysis entirely, arriving as sensation rather than thought",
                "a merging of attention and subject — the distinction between the observer and the observed becoming unclear",
                "the inability to paraphrase what is being experienced, as if language has been temporarily revoked",
                "surfacing briefly — a flicker of awareness that time is passing, that the world exists — before being pulled back under",
                "the reluctance to return, the mind resisting the break the way a dreamer resists waking",
                "a sense of having been somewhere else, the re-entry to ordinary consciousness carrying the slight confusion of jet lag",
                "the thought arriving only after: how long was I gone? where did the time go?",
                "a residual shimmer of the experience lasting for hours — the song still playing internally, the image still glowing behind the eyes",
            ]),
            ("Suppression Cues", vec![
                "shaking the head to break the spell, the physical snap like surfacing from underwater",
                "forcing the eyes to blink, then look away — the deliberate severance of the gaze",
                "checking the time and feeling the shock of how much has passed — the guilt arriving immediately after the pleasure",
                "standing up abruptly, creating physical distance between the self and the source of the absorption",
                "laughing nervously at how completely you checked out: 'sorry, I was a million miles away'",
                "closing the book, pausing the music, shutting the laptop — cutting the source before it swallows another hour",
                "turning to practical matters with forced briskness — the re-entry to productivity as penance for the indulgence",
                "dismissing the depth of the experience: 'I just zoned out for a second' when the reality was total immersion",
                "resisting the urge to return — the book left on the table, the gallery door walked past, the song not replayed — because surrendering again feels dangerous",
                "setting a timer or alarm before allowing the next immersion — the pragmatic compromise with a self that knows it cannot be trusted to surface on its own",
            ]),
        ]),
        // ── Love & Connection Family ──
        ("Love", "The deepest attachment and desire for another's wellbeing — both feeling and commitment", vec![
            ("Physical Signals", vec![
                // ── New Love / Falling ──
                "pupils dilating when they enter the room — the eyes darkening, opening wider, hungry for more light, more of them",
                "the gravitational lean: the body angling toward them without instruction, closing distance one unconscious inch at a time",
                "mirroring their posture, their gestures, their rhythm — the body rehearsing a duet before the mind agrees to dance",
                "finding excuses to touch — adjusting a collar, brushing away an eyelash, a hand on the forearm that stays a beat too long",
                "the voice changing in their presence: softer, lower, warmer, pitched for the space between two people",
                "color rising in the face and neck when they look at you directly, the blush arriving before the thought",
                "the inability to stop smiling — not a performed smile but the involuntary one the face makes when it is simply glad",
                "preening without awareness: smoothing hair, straightening clothes, checking the reflection in a window",
                "breath catching when they appear unexpectedly, the lungs forgetting their rhythm",
                "watching them across a room and losing track of the conversation happening in front of you",
                // ── Deep Love / Established ──
                "the hand settling on the small of their back as they pass — the touch so habitual it's almost unconscious",
                "finishing each other's sentences, or starting to speak at the same time and laughing at the collision",
                "breathing in sync without knowing it — the rise and fall of two chests keeping the same time",
                "a whole conversation conducted in a single glance across a crowded room: should we go? yes. now? five minutes",
                "the absent-minded touch while reading, cooking, driving — a hand finding a knee, a foot nudging a foot, contact maintained without attention",
                "adjusting their scarf, their hat, the blanket over their legs — the care expressed through small, constant tending",
                "sleeping curved around each other, the bodies finding their shape automatically in the dark",
                "the particular way of saying their name — the sound of it worn smooth by years of use, comfortable as a held stone",
                "sitting in comfortable silence, neither needing to speak, the quiet between them furnished and warm",
                "looking at them while they're absorbed in something else and feeling the full weight of the feeling arrive, unprompted, unrequested",
                // ── Universal ──
                "the hug that doesn't let go first — the arms tightening rather than loosening, the face pressed into the shoulder",
                "standing so close that no observer could mistake the two bodies for anything but a pair",
                "laughing at their joke even when it isn't funny, the delight in the *them* of it rather than the humor",
                "tucking a strand of hair behind their ear with a tenderness that makes the gesture feel like a whole sentence",
                "the forehead pressed gently against theirs — the closeness that wants nothing except to be close",
            ]),
            ("Internal Sensations", vec![
                // ── New Love ──
                "the dopamine flood of early love — the reward circuitry firing so hard the world sharpens, colors brighten, music sounds better",
                "the heart rate spiking at a notification, a footstep, the sound of their key in the door",
                "a warmth in the chest so intense it feels like the ribcage might not be large enough",
                "the stomach flipping at their touch — not nausea but the body's register of something seismic happening at the cellular level",
                "skin becoming hypersensitive where they touched — the ghost of contact lingering for minutes, the nerve endings replaying",
                "the inability to eat, to sleep, to concentrate — the body's systems rerouted to a single obsession",
                // ── Deep Love ──
                "oxytocin settling in like a tide rather than a wave — the bonding hormone replacing dopamine's fireworks with something steadier and deeper",
                "the heart slowing in their presence, the blood pressure dropping — the parasympathetic calm of true safety",
                "warmth that doesn't blaze but glows — the low, constant heat of an ember rather than a match",
                "the body relaxing fully only in their presence — muscles releasing a tension you didn't know you were carrying until they walked in",
                "a physical ache when separated — not dramatic but persistent, like a low note humming just below the threshold of hearing",
                // ── Universal ──
                "the lump in the throat when they do something kind without being asked — the tenderness arriving with physical force",
                "the jolt of fear at the thought of losing them — sharp, electric, out of proportion, the body's way of saying what the mouth hasn't",
                "their scent triggering an entire neurological cascade: home, safety, desire, belonging — all arriving simultaneously through the nose",
                "the chest tightening when they're hurting — their pain registering in your body as if the nervous systems have merged",
            ]),
            ("Mental Responses", vec![
                // ── New Love ──
                "the prefrontal cortex dimming — judgment suspended, flaws invisible, the beloved idealized beyond all reason",
                "the compulsive mental replay of every conversation, every glance, every accidental touch — searching for evidence",
                "the inability to think about anything else for more than ninety seconds before the mind swings back to them",
                "fantasizing about the future in granular detail: the apartment, the morning routine, the way they'll look in ten years",
                // ── Deep Love ──
                "knowing how they take their coffee, which side they sleep on, the sound they make before they sneeze — the encyclopedia of intimacy",
                "thinking 'I need to tell them about this' as the first response to anything interesting, beautiful, or strange",
                "the fierce protectiveness that arrives without warning — the willingness to stand between them and anything",
                "seeing them through two lenses at once: the person they are now and every version you've watched them be",
                "choosing them again — not the dramatic choice of vows but the daily, mundane, Tuesday-morning choice of staying",
                // ── Universal ──
                "the vulnerability of being fully known and the terror and relief of it existing simultaneously",
                "the thought arriving with the quiet force of something long understood: this person is where I live",
                "admiring not what they do for you but who they are when they think no one is watching",
                "the awareness that love is not a feeling but a thousand small decisions made in the same direction",
                "gratitude arriving in the ordinary moments — watching them laugh, watching them sleep, watching them exist",
            ]),
            ("Suppression Cues", vec![
                // ── Fighting the Feeling ──
                "pulling back from a touch that lasted too long, the hand retreating as if burned by its own warmth",
                "looking away when they catch you staring — the gaze broken too quickly, the evidence of looking itself a confession",
                "crossing arms, creating distance, leaning against the opposite wall — the body building fortifications the heart has already breached",
                "speaking in a clipped, casual tone to counteract the softness threatening to leak through the voice",
                "finding flaws to focus on — cataloguing reasons this is a bad idea while the body leans in anyway",
                "leaving the room rather than staying close enough to say the thing that can't be unsaid",
                // ── Hiding Depth in Established Love ──
                "deflecting tenderness with humor: a joke where a declaration should be, a laugh where a kiss belongs",
                "saying 'you too' instead of the full sentence, the brevity a shield against the enormity of the feeling",
                "showing love through action rather than words — the car filled with gas, the prescription picked up, the coffee made before they wake — the declaration disguised as logistics",
                "performing irritation at a habit that is secretly beloved — the complaint that is actually a love letter read backward",
                "telling the story of how you met with practiced nonchalance, as if the most important thing that ever happened to you were only moderately interesting",
                "the 'I love you' said while leaving the room, thrown over the shoulder, kept light — because saying it face to face, still, after all this time, is almost too much",
            ]),
        ]),
        ("Tenderness", "The soft, careful feeling toward someone fragile or precious — love at its most gentle", vec![
            ("Physical Signals", vec![
                "touching with just the fingertips — the lightest possible contact, as if the person were made of something that could break",
                "brushing hair from a sleeping face with a slowness that suggests the gesture is more for the toucher than the touched",
                "cupping a face in both hands, thumbs tracing the cheekbones, the hold careful and reverent",
                "the voice dropping to its softest register — not a whisper but a hush, the volume of a bedside",
                "pulling a blanket up over someone who has fallen asleep, tucking the edges with the precision of a ritual",
                "pressing lips to a forehead — not a kiss of passion but of benediction, lingering and warm",
                "holding something small and living — a hand, a bird, a newborn — with the careful grip of someone aware of their own strength",
                "eyes softening until the gaze is almost liquid, the hard edges of everyday looking dissolved",
                "stroking a thumb back and forth across the knuckles of a held hand, the rhythm slow and hypnotic",
                "resting a chin on top of someone's head, the arms around them loose but encompassing",
                "straightening a collar, buttoning a coat, adjusting a hat — the small acts of tending that say 'I see you and you matter'",
                "carrying a sleeping child with exaggerated care, each step measured, the body rearranged around its precious cargo",
                "wiping a tear from someone's face with a thumb rather than offering a tissue — the preference for touch over efficiency",
                "speaking to an animal, a baby, a wounded thing in a voice that has no irony in it — pure, unguarded gentleness",
                "the hand hovering before it lands — a moment of hesitation born not from uncertainty but from the desire to be exactly right",
            ]),
            ("Internal Sensations", vec![
                "oxytocin flooding the system — the caregiving hormone activating, cortisol dropping, the body entering its tending mode",
                "a warmth in the chest that doesn't blaze or radiate but pools, gentle and contained, like cupped water",
                "the heart rate slowing to its most peaceful rhythm, the pulse steady as a lullaby",
                "a softening behind the eyes — the sting of tears that come not from sadness but from the unbearable sweetness of something fragile",
                "the hands becoming acutely sensitive, the fingertips registering every texture, every temperature, every micro-movement of the thing being held",
                "a loosening in the throat, the muscles opening rather than closing — this emotion does not need to be swallowed, only expressed",
                "the whole body quieting, the breathing shallow and careful, as if too deep a breath might disturb the moment",
                "a fullness in the chest that feels breakable — the awareness that to feel this much for something so small is to be exposed",
                "the jaw unclenching, the brow smoothing, the face releasing every trace of hardness — the muscles surrendering to softness",
                "warmth concentrating in the palms, the hands becoming the warmest part of the body, ready to hold and heal",
            ]),
            ("Mental Responses", vec![
                "the fierce, quiet thought: I will protect this — not as a vow but as a fact, as involuntary as breathing",
                "seeing the other person not as they present themselves but as they are underneath — the vulnerability they try to hide, visible and dear",
                "a heightened awareness of their fragility — the thin skin at the wrist, the soft fontanelle, the unguarded sleep, the ways they can be hurt",
                "the desire to make the world gentler around them — to quiet the noise, to soften the light, to be the buffer between them and everything sharp",
                "time slowing to the pace of the gesture — the world outside this room, this bed, this held hand becoming irrelevant",
                "the thought that their trust is the most valuable thing you've ever been given, and the most terrifying",
                "admiring their courage without saying so — recognizing what it costs them to be this open, this undefended",
                "the mental inventory of what they need: are they warm enough, have they eaten, when did they last sleep — the caretaker's checklist running quietly in the background",
                "a willingness to be inconvenienced, to stay awake, to be uncomfortable, to give whatever the moment asks without counting the cost",
                "the awareness that tenderness makes you vulnerable too — to care this much for something fragile is to carry the possibility of its breaking",
            ]),
            ("Suppression Cues", vec![
                "pulling the hand back and shoving it into a pocket, the tenderness too raw to display",
                "roughening the voice on purpose — the gruff 'you okay?' that hides the gentle 'please be okay'",
                "converting the care into practical action: making food, fixing something, solving the problem — because sitting with the softness is too exposing",
                "making a joke to break the intensity: 'don't get used to it' — the armor rebuilt with humor the instant it was lowered",
                "touching briefly then withdrawing, as if the contact burned — not from rejection but from the vulnerability it opened",
                "looking away when the tenderness threatens to become visible on the face",
                "calling it something else: 'I was just checking' instead of 'I was worried', 'it's nothing' instead of 'you're everything'",
                "performing irritation over the concern: 'you need to take better care of yourself' spoken sharply, the worry dressed as a scold",
                "the tight nod and quick exit from the room after doing something deeply kind — refusing to stay for the thank-you",
                "saying 'anyone would have done it' when the truth is that no one else thought to, and you couldn't have stopped yourself if you tried",
            ]),
        ]),
        ("Longing", "The ache of desire for someone or something absent or out of reach", vec![
            ("Physical Signals", vec![
                "staring out a window without seeing the glass, the gaze aimed at a distance the room can't contain",
                "reaching for a phone, holding it, not dialing — the hand performing the first half of a connection it won't complete",
                "pressing a palm flat against a surface — a door, a wall, a window — as if touch could reach through to the other side",
                "re-reading old messages, scrolling slowly, the thumb hesitating over certain lines",
                "holding an object that belongs to the absent person — a scarf, a ring, a book — the grip both possessive and tender",
                "the deep, slow sigh that empties the lungs completely, the sound of the body releasing what the mind won't",
                "lying on their side of the bed, face pressed into the pillow that still holds the shape of them",
                "writing a letter, an email, a message — and saving it to drafts, the words too honest to send",
                "pausing mid-step at a place that holds a shared memory — the restaurant, the corner, the bench — the feet stopping before the mind catches up",
                "eyes closing when a certain song plays, the face tipping upward, the body offering itself to the memory",
                "fingertips tracing the outline of a photograph, following the shape of a face that can't be touched",
                "the lips parting slightly, as if about to speak to someone who isn't there",
                "standing in a doorway, neither entering nor leaving, the body suspended between here and the place it wants to be",
                "cooking their favorite meal for no one — the act of preparation a ritual of proximity when proximity is impossible",
            ]),
            ("Internal Sensations", vec![
                "a deep ache behind the sternum — not sharp, not acute, but constant, as if something essential has been removed and the space refuses to close",
                "the reward system firing at memories the way it fires at the real thing — the nucleus accumbens craving what's absent, the brain unable to distinguish between wanting and having",
                "a tightness in the throat that comes in waves, each wave triggered by a detail: a scent, a sound, a time of day",
                "the chest feeling hollow and full simultaneously — empty of the person, full of the wanting",
                "a warmth that rises and has nowhere to go — the body preparing for a reunion that isn't coming",
                "the stomach pulling inward, a contraction that feels like hunger but can't be fed",
                "skin aching for contact — the specific touch of a specific person, no substitute acceptable",
                "a restlessness in the limbs that no amount of movement resolves — the body wanting to go somewhere the mind knows it can't",
                "the dopamine system stuck in craving mode — the same circuitry as addiction, the same endless reach for the absent thing",
                "the physical sensation of distance — felt not as a fact but as a weight, measurable in the heaviness of the arms and the slowness of the days",
            ]),
            ("Mental Responses", vec![
                "the mind returning to the same memory on a loop, each replay a hit of the drug, each return making the absence sharper",
                "imaginary conversations with the absent person — full dialogues, scripted and re-scripted, the responses imagined in their exact voice",
                "the mental math of distance: how many miles, how many hours, how many days until—",
                "the ache of almost — almost calling, almost going, almost saying it — the near-misses accumulating into a private library of regret",
                "fantasizing not about grand gestures but about ordinary proximity: sitting beside them, hearing them breathe, watching them read",
                "the Portuguese *saudade*, the German *Sehnsucht* — the longing so specific it requires a word no English equivalent can hold",
                "the awareness that the thing being longed for may be idealized, and the longing surviving the awareness unchanged",
                "time bending around the absence — every clock-check a measurement of the gap, every sunset a countdown to or from something",
                "the thought arriving with utter simplicity, devastating in its plainness: I miss you",
                "wondering if they feel it too, and the secondary ache of not knowing",
            ]),
            ("Suppression Cues", vec![
                "putting the phone face-down and walking to another room, as if distance from the device is distance from the feeling",
                "filling the hours with noise and motion — plans, errands, people — because stillness is where the longing lives",
                "deleting the draft message, the unsent letter, the half-dialed number — severing the circuit before the current runs",
                "telling yourself it will pass, knowing it will, knowing it won't, saying it anyway",
                "unfollowing, muting, archiving — the digital housekeeping of someone trying to starve an emotion of its fuel",
                "redirecting the conversation when someone mentions them: 'anyway, what were you saying?'",
                "throwing yourself into work, into exercise, into anything with a deadline — urgency as a substitute for presence",
                "performing contentment with the present: 'I'm fine where I am' — the voice steady, the hands gripping the edge of the table",
                "replacing the specific longing with a general restlessness, refusing to name what's missing because naming it makes it real",
                "going to sleep early because consciousness is where the wanting happens, and sleep is the only reprieve that doesn't require lying",
            ]),
        ]),
        ("Desire", "The pull toward someone or something — wanting that has heat and urgency in it", vec![
            ("Physical Signals", vec![
                // ── The Gaze ──
                "eyes tracking them across the room with the slow, deliberate attention of someone memorizing a map",
                "the gaze traveling — from eyes to mouth to throat to collarbone — the look that is itself a kind of touch",
                "pupils dilating until the eyes darken, the body signaling what the mouth hasn't said",
                "holding eye contact one beat past comfortable, then two, the silence between them thickening",
                "looking away when caught staring, then looking back — the dance of want and caution",
                // ── The Body Orienting ──
                "leaning in until the space between them is charged, electric, a gap that hums",
                "feet pointing toward them like compass needles, the body orienting before the mind consents",
                "the unconscious mirroring — when they lean forward, you lean forward; when they touch their neck, your hand rises to yours",
                "standing too close in a room full of space, the proximity a choice that pretends to be accidental",
                "turning the body fully to face them, shoulders squared, the posture of total attention",
                // ── Touch and Self-Touch ──
                "fingers trailing along a collarbone, adjusting a neckline, the hand drawing attention to exposed skin without conscious intent",
                "the brush of a hand that lingers — on a wrist, a waist, the small of a back — the touch that asks a question",
                "wetting the lips, the tongue running along the lower lip in a gesture as old as the species",
                "playing with a necklace, a button, a glass stem — the hands performing rehearsals of touch on substitute objects",
                "the accidental contact that sends both of them still — a knee against a knee under the table, a reaching for the same doorknob",
                // ── Voice and Breath ──
                "the voice dropping lower, softer, pitched for the distance between two mouths",
                "breath quickening in their proximity — shallow, warm, audible in the quiet between words",
                "laughing at something that isn't funny, the sound a bridge to closeness rather than humor",
                "speaking slowly, letting certain words land with more weight than they need — the ordinary sentence made intimate by delivery",
                // ── The Body's Readiness ──
                "skin flushing across the chest, the neck, the ears — blood rushing to the surface as if to meet the anticipated touch",
                "the subtle shift in posture — spine straightening, chest lifting, stomach drawn in — the body presenting itself",
                "a hand resting on the table, palm up, fingers loose — the posture of offering, of availability",
                "the swallow visible in the throat when they step closer, the body's acknowledgment of what's approaching",
            ]),
            ("Internal Sensations", vec![
                "heat pooling low in the abdomen — not metaphorical but literal, the blood redirecting, the body preparing",
                "the dopamine-norepinephrine surge hitting like a current — the same circuitry as a drug, the same compulsive focus",
                "skin becoming hypersensitive, every nerve ending amplified — the brush of fabric, the movement of air, all registering as almost-touch",
                "the heart not hammering but thickening its beat — heavier, more deliberate, each pulse felt in the throat and the wrists",
                "breath becoming conscious — the awareness of breathing itself, the regulation of it, as if the body knows it might need to share this air",
                "a tightness coiling in the chest and stomach, the muscles winding around a center of gravity that has shifted toward the other person",
                "the mouth going dry, the throat clicking on a swallow that has nothing to do with thirst",
                "temperature rising from the inside — a flush spreading across the chest and up the neck, the body's furnace answering a call",
                "the skin where they touched still humming minutes later, the nerve endings replaying the contact on a loop",
                "a vertigo that has nothing to do with height — the world narrowing to the space between two bodies, everything else going soft-focus",
                "the ache of proximity — close enough to smell their skin, their hair, the warmth rising off their body — and not yet close enough",
                "testosterone and dopamine flooding the limbic system — the ancient brain overriding the modern one, the body making arguments the mind can't answer",
            ]),
            ("Mental Responses", vec![
                "awareness sharpening to a single point — noticing the exact way they hold a glass, the precise curve of a lip, the movement of a tendon in the wrist",
                "the conversation happening on two tracks: the words being said and the negotiation being conducted underneath them",
                "the inability to think more than thirty seconds ahead or behind — past and future collapsed to this room, this moment, this person",
                "fantasizing in fragments — a hand on a hip, a mouth on a throat, the weight of a body — the images arriving uninvited and refusing to leave",
                "the mental arithmetic of possibility: are we alone, how long do we have, does this mean what I think it means",
                "the heightened reading of every signal — the tilt of a head, the pitch of a laugh, the duration of a touch — all parsed for intention",
                "thoughts becoming simpler, more direct, the sophisticated mind reduced to elemental want: closer, more, now",
                "the awareness that what you're feeling is visible — in the eyes, in the voice, in the way you keep finding reasons to be near them",
                "the dangerous, intoxicating erasure of consequences — the future temporarily irrelevant, the present moment all that exists",
                "the thought arriving with the simplicity of an animal instinct, stripped of language: I want this person",
            ]),
            ("Suppression Cues", vec![
                // ── Physical Distance ──
                "stepping back when the proximity becomes too charged — creating space the body immediately wants to close",
                "crossing arms over the chest, building a barrier of bone and muscle against the pull",
                "turning to face the room, the window, anything other than the person generating the heat",
                "picking up a glass, a phone, a book — giving the hands an occupation that prevents them from reaching",
                // ── Voice and Behavior ──
                "forcing the voice back to its normal register after it's dropped into the intimate range",
                "introducing a third topic, a third person, a third anything — triangulating to defuse the charge between two",
                "laughing too loudly, the sound a detonation meant to break the tension before it breaks something else",
                "checking a watch, a phone, the door — performing distraction when every cell is oriented in one direction",
                // ── Internal Override ──
                "the mental inventory of reasons this is a bad idea, recited with increasing desperation as the body ignores every item",
                "clenching the jaw to keep from saying the thing that would change everything",
                "redirecting the energy into something safe — a workout, a cold shower, a long walk — the body needing to burn what can't be consumed",
                "the careful, deliberate goodnight said from a safe distance, the door closed with exactly the force needed to stay on this side of it",
            ]),
        ]),
        ("Compassion", "The ache of feeling another's pain alongside them, paired with the impulse to help", vec![
            ("Physical Signals", vec![
                "the face softening all at once — brow smoothing, jaw loosening, the expression becoming open and unguarded",
                "leaning forward in the chair, the body closing the distance toward someone in pain",
                "a hand placed gently on an arm, a shoulder, a back — the touch offered not as a fix but as a witness",
                "eyes holding the other person's gaze steadily, not looking away from the pain, not flinching from it",
                "nodding slowly, continuously, the head moving in the rhythm of someone who is truly listening",
                "matching the other person's breathing without awareness — the bodies syncing, the distress shared through the lungs",
                "kneeling or sitting beside someone rather than standing above them — choosing the lower position, the equal one",
                "pulling someone close without asking, the arms wrapping around them in the quiet understanding that words are insufficient",
                "eyes filling with tears that belong to someone else's story — crying not for your own loss but because theirs has entered you",
                "the voice dropping to its gentlest register, the words spoken slowly, as if each one were being chosen for its softness",
                "offering a glass of water, a tissue, a blanket — the hands doing what the heart can't say",
                "sitting in silence beside someone because the most compassionate thing is sometimes simply staying",
                "the palm pressed flat against the other person's back between the shoulder blades — the steady, grounding pressure that says 'I'm here'",
                "brushing hair from a tear-stained face with the back of a finger — the tenderness that is compassion's signature gesture",
            ]),
            ("Internal Sensations", vec![
                "a warm expansion in the chest — the vagus nerve activating, the specific sensation Porges called the physiological signature of compassion",
                "the heart rate slowing, not rising — the parasympathetic system engaging, the body calming itself to be present rather than fleeing from the pain",
                "oxytocin releasing, buffering the distress of witnessing suffering — making it possible to approach rather than recoil",
                "the throat aching with a lump that is not quite sadness and not quite love but something between — the body's response to proximity to pain",
                "a heaviness settling in the chest that is not depression but *taking on* — the physical sensation of sharing a burden",
                "the eyes stinging, the tears arriving from resonance rather than personal grief — the nervous system mirroring what it sees",
                "a softening through the entire body — the muscles releasing their defensive posture, the armor coming off",
                "warmth concentrating in the palms, the hands becoming ready, the body's way of preparing to hold something fragile",
                "a tug in the solar plexus — the visceral pull toward someone who is hurting, as instinctive as a parent turning at a child's cry",
                "the breath deepening, the body grounding itself, the nervous system shifting into tend-and-befriend rather than fight-or-flight",
            ]),
            ("Mental Responses", vec![
                "the instant, involuntary thought: what do they need, and what can I do",
                "seeing past the surface to the pain underneath — reading the real story in the eyes, the posture, the things not being said",
                "the ego quieting, the self-concern receding — this moment is not about you, and the mind accepts that without resistance",
                "imagining yourself in their position — not as an exercise but as a flash of genuine understanding that arrives with physical force",
                "the fierce protectiveness rising alongside the gentleness: no one should have to feel this alone",
                "the awareness that fixing is not the same as helping — that sometimes the most compassionate act is to sit in the pain without trying to solve it",
                "resisting the urge to compare their pain to your own — staying with *their* experience rather than translating it into yours",
                "the willingness to be uncomfortable, to hear the hard thing, to not look away from what is ugly or frightening or raw",
                "the thought arriving with clarity: I cannot take this from them, but I can carry it with them",
                "a sudden, clarifying perspective on your own complaints — the recalibration that someone else's suffering sometimes provides, humbling and focusing",
            ]),
            ("Suppression Cues", vec![
                "maintaining a professional distance when the instinct is to reach out and hold — the hand staying at one's side by force of will",
                "speaking in a calm, measured tone while the chest aches with what wants to be expressed",
                "offering practical solutions instead of emotional presence — 'have you tried...' deployed as a shield against the vulnerability of simply feeling",
                "clearing the throat and redirecting to logistics: 'what's the plan' instead of 'how are you, really'",
                "looking away from the suffering to compose the face — because being visibly moved feels like making their pain about you",
                "holding back the tears because crying would shift the attention, and this moment belongs to them",
                "performing detachment in a professional setting — the doctor, the soldier, the first responder — compartmentalizing so the hands can stay steady",
                "turning the compassion into anger at the cause rather than sitting with the grief of the effect — rage as a more comfortable container",
                "limiting exposure — leaving the room, changing the channel, closing the article — because the open heart can only take so much before it must close to survive",
                "the private breakdown later: the shower, the car, the walk alone — where the compassion that was held in check can finally move through the body without an audience",
            ]),
        ]),
        ("Gratitude", "The warm recognition that something good came from outside yourself", vec![
            ("Physical Signals", vec![
                "a hand rising to the chest, palm flat over the heart — the instinctive gesture of receiving something that matters",
                "eyes filling suddenly, the tears arriving before the thank-you can form — overwhelmed not by sadness but by being given to",
                "reaching for the other person's hand and holding it with both of yours, the grip warm and lingering",
                "the head bowing slightly — not in deference but in the quiet recognition of something larger than the moment",
                "pressing fingers to smiling lips, the expression caught between laughter and tears",
                "pulling someone into a hug that goes on longer than expected, the arms tightening rather than releasing",
                "the voice cracking on the 'thank you,' the two words carrying more weight than the throat can manage in one pass",
                "smiling with the whole face — eyes crinkling, cheeks lifting, the expression unguarded and real",
                "returning to the gift, the letter, the gesture again and again — touching it, reading it, looking at it, unable to put it down",
                "making eye contact and holding it, the gaze steady and warm, the look that says 'I see what you did and it mattered'",
                "writing a note by hand when an email would suffice — the effort itself a form of thanks",
                "paying it forward immediately — holding a door, leaving a larger tip, helping a stranger — the gratitude overflowing its original container",
                "tracing a finger across an inscription, a name, a date — gratitude directed at someone who can no longer receive it",
            ]),
            ("Internal Sensations", vec![
                "a warmth blooming in the center of the chest — dopamine, serotonin, and oxytocin releasing simultaneously, the brain's triple reward for being given to",
                "the throat tightening with the effort of holding an emotion that wants to be larger than the body can contain",
                "the anxiety system quieting — the amygdala dampening, the stress hormones receding, the nervous system recognizing safety in generosity",
                "a lightness spreading through the limbs, the body physically buoyed by the recognition that it is not alone",
                "eyes stinging with the particular tears of someone who did not expect kindness and is undone by receiving it",
                "a swelling behind the ribs that is not pressure but fullness — the sensation of being filled by something that came from outside the self",
                "the heart beating with a steady, warm rhythm — not the spike of excitement but the deep pulse of connection",
                "a loosening in the shoulders, the jaw, the hands — the body releasing tension it was holding against a world it now trusts slightly more",
                "the sensation of debt — gentle, willing, warm — the awareness that something was given that cannot be fully repaid, and the desire to try anyway",
                "warmth reaching the face, the cheeks flushing not from embarrassment but from the exposure of being seen and helped",
            ]),
            ("Mental Responses", vec![
                "the thought arriving with startling clarity: I didn't earn this, and it was given anyway",
                "the mental replay of the moment — the gift, the words, the look on their face — revisited not to analyze but to savor",
                "the recalibration of the world: it is kinder than I thought, people are better than I feared",
                "a desire to be worthy of what was given — not through repayment but through becoming someone who gives the same way",
                "suddenly noticing other things to be grateful for, the feeling spreading outward from the original source like ripples in water",
                "the humility of receiving — the recognition that strength is not only in giving but in allowing oneself to be helped",
                "thinking of the person who gave and feeling a warmth toward them that is close to love but softer, less possessive",
                "the awareness that this moment has changed something — a small permanent shift in how the world is perceived",
                "wanting to tell someone about it — not to boast but to share the evidence that goodness exists and arrived, here, for you",
                "the thought 'I will remember this' — not as a promise but as a certainty, the moment already filing itself in the permanent collection",
            ]),
            ("Suppression Cues", vec![
                "clearing the throat and saying 'thanks' briskly, as if the word were a full stop rather than the beginning of what wants to be said",
                "blinking rapidly to push back the tears before they become visible — gratitude's tears being somehow more embarrassing than grief's",
                "deflecting the gift or compliment with humor: 'you shouldn't have' meaning 'I don't know what to do with this much kindness'",
                "redirecting the attention back to the giver: 'no, you're the one who—' as if accepting the gratitude fully would tip some invisible scale",
                "the tight nod and the quick look away — the composure maintained by not examining the feeling too closely",
                "minimizing the impact: 'it was no big deal' spoken about a gesture that is clearly rearranging something inside",
                "busying the hands with the wrapping, the envelope, the practical details — because sitting still with the emotion is too much",
                "postponing the real thank-you for later, for a letter, for a moment alone — because expressing it face to face requires a vulnerability the public moment won't allow",
                "performing casual ease while the chest is tight with unexpressed feeling: 'yeah, that was really nice of them' — the understatement of the decade",
                "saying 'I owe you one' — converting the warmth into a transaction because a debt is easier to carry than a grace",
            ]),
        ]),
        // ── Shame & Self-Conscious Family ──
        ("Shame", "The total-self indictment that you are fundamentally flawed or unworthy — it wants to hide", vec![
            ("Physical Signals", vec![
                "the head dropping forward and down, chin nearly touching the chest, as if the neck has lost its scaffolding",
                "eyes fixed on the ground, unable to lift even when directly addressed — the gaze pinned there by gravity",
                "shoulders curling inward and rising toward the ears, the body trying to fold itself into the smallest possible shape",
                "a hot flush climbing the neck and flooding the face, the skin betraying what the mouth refuses to say",
                "covering the face with both hands, fingers pressed hard into the eye sockets, hiding behind the only shield available",
                "the voice shrinking to a near-whisper, words trailing off before they finish, as if each one costs too much",
                "turning the body away mid-conversation — a quarter-turn, then a half — angling toward the nearest exit",
                "arms crossing tight against the chest, hands gripping opposite elbows, a self-embrace that offers no comfort",
                "picking at cuticles, sleeves, skin — small, repetitive destructions the fingers perform without permission",
                "the jaw clamping shut between sentences, teeth pressing together as if locking the next confession inside",
                "a visible flinch at one's own name being spoken aloud, the sound landing like an accusation",
                "shuffling steps, gait reduced to the minimum, feet barely clearing the floor — moving like someone trying not to be noticed",
                "sitting with knees drawn up to the chest, arms wrapped around the shins, compressing into the smallest possible ball",
                "the hand rising to touch the back of the neck or cover the throat, a protective gesture over vulnerable places",
                "quick, darting glances to see who is watching before the eyes snap back to the ground",
                "pulling a hood, collar, or hair forward across the face, improvising a barrier between self and world",
            ]),
            ("Internal Sensations", vec![
                "a searing heat radiating outward from the sternum, as if a coal has been lodged behind the breastbone",
                "the skin crawling — a desperate, full-body sensation of wanting to unzip oneself and step out",
                "a sinking weight in the stomach, not nausea but a stone dropping through the gut with no bottom",
                "the throat sealing shut, swallowing blocked by a lump that feels solid and permanent",
                "a cold sweat breaking along the hairline and palms despite the face burning hot — the body at war with itself",
                "the chest compressing inward, ribs tightening as if the skeleton is trying to collapse around the heart",
                "a dizzy, dissociative float — the mind pulling up and away from the body that has become the source of the pain",
                "cortisol flooding the system, the same alarm chemicals as physical danger but aimed at the self",
                "the heart pounding in the ears, loud enough to drown out whatever is being said",
                "a numb tingling in the extremities as blood retreats to the core, the body's ancient signal to play dead",
                "the muscles going slack all at once — not relaxation but collapse, the body surrendering its own posture",
                "a raw, exposed sensation across the entire surface of the skin, as if a protective layer has been stripped away",
            ]),
            ("Mental Responses", vec![
                "the conviction that this single moment has revealed the permanent, irredeemable truth of what you are",
                "a desperate mental catalog of every exit — door, excuse, sudden illness — anything to stop being seen",
                "the memory replaying on an immediate, involuntary loop, each repetition adding detail and sharpening the sting",
                "the certainty that everyone present can see through to the defective core and always could",
                "thought collapsing from the specific event to a global verdict: not 'I failed' but 'I am a failure'",
                "a frantic desire to rewrite the last five minutes, the mind offering impossible bargains with time",
                "the inner voice switching from narrator to prosecutor, listing prior evidence of unworthiness",
                "a strange, detached observation of oneself from the outside — watching the pathetic figure you've become",
                "the urge to confess everything, to preemptively expose every flaw before someone else does",
                "thoughts fragmenting — unable to construct a full sentence internally, just flashes of the worst moment",
                "the magical belief that if you hold still enough, stay quiet enough, you might become invisible",
                "fantasies of being someone else entirely — not a better version of yourself but a different person altogether",
            ]),
            ("Suppression Cues", vec![
                "forcing the chin up and the shoulders back, manually overriding every muscle that wants to curl inward",
                "steering the conversation to someone else's failure, redirecting the spotlight with surgical precision",
                "a sudden, aggressive performance of confidence — louder voice, bigger gestures, a mask hammered into place",
                "laughing first, hardest, making a joke of the thing before anyone else can, claiming the humiliation as comedy",
                "the strategic deployment of anger — shame's bodyguard, arriving loud to cover what's really happening",
                "retreating to a bathroom, a car, any private space to let the face do what it's been fighting not to do",
                "busying the hands with a phone, a drink, a task — activity as camouflage for the internal collapse",
                "intellectualizing the moment, analyzing it in the third person, converting feeling into clinical distance",
                "apologizing preemptively for everything — the weather, the seating, the interruption — a constant stream of sorry that never addresses the real source",
                "going very still and very quiet, the social equivalent of a prey animal freezing in tall grass",
                "smiling with the mouth while the eyes stay flat and unfocused, the performance requiring every ounce of remaining energy",
            ]),
        ]),
        ("Guilt", "The action-specific discomfort of having done something wrong — it wants to repair", vec![
            ("Physical Signals", vec![
                "eyes dropping to the floor when the subject comes up, the gaze physically unable to hold the other person's",
                "fidgeting with hands — picking at nails, twisting a ring, rubbing a thumb across the knuckles — the body's attempt to self-soothe",
                "shoulders hunching forward, the posture collapsing inward as if trying to occupy less space in the world",
                "touching the face compulsively — rubbing the chin, scratching the nose, covering the mouth — the hand trying to hide what the face might reveal",
                "the voice going quieter, thinner, the volume pulled down as if speaking at full strength would draw attention to the wrong person",
                "overcompensating with helpfulness — offering too much, doing too much, the favors a currency of atonement",
                "flinching when the wronged person enters the room, the body bracing before the mind can compose itself",
                "avoiding the person you've hurt — taking a different route, leaving a room when they enter, the geography of guilt",
                "apologizing too often, for things that don't require apology, the word 'sorry' leaking out like a pressure valve",
                "the darting glance toward the person you've wronged — checking their expression, reading their mood, monitoring for the moment they find out",
                "restless sleep visible in the morning: dark circles, pallor, the face of someone who spent the night in dialogue with their own ceiling",
                "the hand reaching for the phone to call, to confess, to explain — then pulling back, then reaching again",
                "eating without tasting, or not eating at all — the appetite disrupted by the stomach that won't unclench",
                "sitting very still in the presence of the wronged person, the body careful and contained, as if movement might draw the accusation",
            ]),
            ("Internal Sensations", vec![
                "a weight pressing on the chest — not the hollowness of sadness but the density of something carried, something that belongs to you",
                "the stomach churning in slow, persistent rotations, the nausea of self-knowledge",
                "cortisol elevating from the rumination — the replaying raising stress hormones, the stress disrupting sleep, the disrupted sleep feeding more rumination",
                "a heat in the face and ears when the subject is approached — the blush of someone whose secret is close to the surface",
                "the throat constricting around the confession that wants to come out — the words pressing from the inside like something swallowed alive",
                "a heaviness in the limbs that feels like penance — the body sluggish, weighed down, as if guilt has a mass it adds to every movement",
                "the heart rate spiking at any mention of the topic, any adjacent word, any silence that might contain the question",
                "a specific, localized ache behind the sternum — the physical address where guilt lives in the body",
                "the jaw aching from nighttime clenching, the body processing in sleep what the waking mind refuses to resolve",
                "an exhaustion that comes not from activity but from the constant internal labor of carrying something that should be set down",
            ]),
            ("Mental Responses", vec![
                "the replay loop: the moment of the wrong action playing on repeat, the mind rewinding to the exact second a different choice was possible",
                "the urge to confess — strong, specific, action-oriented — guilt's defining feature, the desire to repair what was broken",
                "bargaining with the timeline: if I had said this instead, if I had waited, if I had been someone else in that moment",
                "the mental rehearsal of the apology — scripted, revised, rehearsed, never delivered, revised again",
                "the question on a loop: do they know? can they tell? is this the conversation where it comes out?",
                "a hyperawareness of the wronged person's emotions — reading their every mood shift as potential evidence of discovery",
                "the inability to enjoy good things — the promotion, the meal, the compliment — because you haven't earned the right to pleasure while the debt remains",
                "the moral ledger running in the background, tallying good deeds against the original wrong, the math never quite balancing",
                "the thought arriving with conviction: I need to make this right — guilt's action tendency, the forward-facing urge that distinguishes it from shame's desire to hide",
                "wondering whether forgiveness is possible, and whether you deserve to ask for it, and whether asking would be selfish too",
            ]),
            ("Suppression Cues", vec![
                "rationalizing: 'anyone would have done the same,' 'they left me no choice,' 'it wasn't that bad' — the arguments constructed against the feeling's verdict",
                "burying the guilt under busyness — working harder, staying later, filling every silence with productivity",
                "deflecting blame outward: finding fault in the wronged person, in the circumstances, in anything that shifts the weight",
                "performing normalcy with precision — the exact right amount of eye contact, the calibrated warmth — because acting innocent is the alternative to confessing",
                "compartmentalizing: storing the guilt in a locked room and functioning as if it doesn't exist, until a song or a silence opens the door",
                "making amends indirectly — anonymous gestures, generalized kindness — atoning without admitting, repairing without confessing",
                "telling a version of the story that omits the crucial detail — the lie of omission that lets you be truthful about everything except the thing that matters",
                "converting guilt into irritation with the wronged person — 'if they weren't so sensitive' — the perverse alchemy of turning accountability into resentment",
                "drinking, scrolling, staying up late — any numbing agent that delays the confrontation between the self and the self",
                "the tight smile when someone praises your character — the specific agony of being seen as good by someone who doesn't know what you did",
            ]),
        ]),
        ("Embarrassment", "The hot, public flush of having been exposed in a socially awkward moment", vec![
            ("Physical Signals", vec![
                "the blush arriving like a lit match — blood rushing to the face, the neck, the ears, visible and uncontrollable",
                "eyes dropping instantly, the gaze hitting the floor as if magnetized",
                "a hand flying up to cover the mouth, the face, the forehead — anything to hide behind",
                "the nervous laugh — quick, breathy, pitched too high — the sound of someone trying to shrink a moment",
                "shoulders pulling up toward the ears, the body trying to retract the head like a turtle",
                "stumbling over words, the sentence falling apart mid-delivery — 'I just — I didn't mean — sorry'",
                "touching the back of the neck, the hot spot where the blush is fiercest and most felt",
                "fidgeting with anything available — a button, a napkin, the hem of a shirt — the hands needing a decoy",
                "the tight, wincing smile of someone enduring public attention they did not invite",
                "turning away, the whole body rotating to present the shoulder rather than the face",
                "tugging at a collar, pulling fabric from flushed skin, the clothing suddenly too warm and too close",
                "glancing around the room to assess who saw, the quick inventory of witnesses",
                "the over-bright 'anyway!' deployed to wrench the conversation onto any other topic",
                "walking too fast away from the scene, the retreat disguised as having somewhere else to be",
                "burying the face in both hands and letting out a muffled groan — the audible surrender to the moment",
            ]),
            ("Internal Sensations", vec![
                "heat flooding the face from the inside — the sympathetic nervous system dilating every blood vessel in the cheeks and ears at once",
                "the stomach dropping, the butterflies arriving in a swarm — digestion slowing as the fight-or-flight response hijacks the blood supply",
                "skin prickling with the awareness of being watched — every pair of eyes in the room felt as a physical pressure",
                "the heart accelerating sharply, pounding in the throat and the temples, audible in the silence that follows the blunder",
                "a wave of heat traveling from the chest up through the neck and into the scalp, the full-body blush arriving in stages",
                "the mouth going dry, the tongue sticking to the palate at the exact moment speaking is required",
                "a tingling in the hands and feet, the blood retreating to the core and the face — the extremities forgotten",
                "the throat tightening, the voice about to crack, the breath too shallow to support steady speech",
                "the particular, exquisite agony of feeling the blush and knowing it is visible — embarrassment compounding embarrassment",
                "a fleeting dizziness, the world tilting slightly, as if the ground has shifted under the social footing",
            ]),
            ("Mental Responses", vec![
                "the instant, desperate wish to rewind — thirty seconds, just thirty seconds, that's all it would take",
                "the thought 'everyone saw' arriving at full volume, whether or not it's true",
                "a rapid mental replay of what just happened, each review making it worse, the cringe deepening with each pass",
                "the wish for the floor to open, the earth to swallow, the fire alarm to go off — any deus ex machina to end this moment",
                "the brain helpfully estimating how many people will remember this, for how long, and in how much detail",
                "self-talk spiraling: 'why did I say that, why did I do that, what is wrong with me'",
                "the awareness that the blush is making it worse and the inability to stop it — the feedback loop of visible self-consciousness",
                "the desperate search for a joke, a deflection, a reframe that could convert the moment from humiliation to humor",
                "the comforting lie assembling itself: 'no one noticed' — spoken internally with no conviction whatsoever",
                "the retroactive mortification arriving hours later in a quiet room — the cringe returning at 2 a.m. with renewed force",
            ]),
            ("Suppression Cues", vec![
                "forcing the head up and the eyes forward, fighting the instinct to look at the ground",
                "laughing first and loudest at yourself — seizing the narrative before someone else can",
                "leaning into the moment with exaggerated self-deprecation: 'well, that happened'",
                "pressing the back of a cold hand against a cheek, willing the blush to cool by sheer physics",
                "speaking faster and more confidently to power through the wobble, the voice overcompensating for the face",
                "acknowledging it briefly and moving on with practiced speed: 'yep, that's me, anyway—'",
                "turning the mistake into a story immediately, the retelling already shaping the humiliation into comedy",
                "excusing yourself to the bathroom to stand in front of the mirror and wait for the red to fade",
                "texting a friend the play-by-play while still in the room — outsourcing the processing, converting the pain to content",
                "telling yourself it will be funny later — and knowing it will, but knowing that 'later' is not now",
            ]),
        ]),
        ("Humiliation", "Embarrassment inflicted by another's deliberate cruelty — a dignity wound", vec![
            ("Physical Signals", vec![
                "the body folding inward — shoulders curling over the chest, head bowing, the posture of someone trying to become invisible",
                "eyes going dull and lifeless, the light draining from them as the dignity drains from the moment",
                "the face flushing crimson, then draining to white — the blood unable to decide between fight and collapse",
                "hands hanging limp at the sides, the arms losing their purpose, the body surrendering its right to gesture",
                "the mouth opening to speak and nothing coming out — the voice stolen, the words swallowed by the magnitude of the exposure",
                "flinching at laughter, even laughter that isn't directed at you — the sound now coded as weapon",
                "hair falling forward across the face, left there as a curtain, the last available hiding place",
                "trembling that begins in the hands and spreads until the whole frame is vibrating with a frequency too low for sound",
                "the slow, mechanical walk away — not fleeing but retreating, each step carrying the weight of every witness's gaze",
                "tears falling without the usual preamble — no quivering lip, no reddening eyes, just liquid appearing on the face as if from nowhere",
                "covering the body with crossed arms, hunched shoulders, hands — a closing-off so complete it looks like the body is trying to fold itself into a drawer",
                "the head shaking in small, involuntary refusals — no, no, no — the body rejecting what is happening before the mind can process it",
                "sitting or crouching on the floor when chairs are available — the body seeking the lowest possible position, unable to hold itself upright",
                "staring at a fixed point — a crack in the wall, a seam in the floor — the gaze anchoring itself to something too small to judge",
            ]),
            ("Internal Sensations", vec![
                "the dorsal vagal shutdown — the nervous system collapsing into freeze, the body going numb, the mind going somewhere else entirely",
                "a heat in the face so intense it feels like being burned from the inside, the skin screaming with visibility",
                "the stomach dropping not in the quick plunge of fear but in the slow, irreversible descent of something falling from a great height",
                "a ringing in the ears that walls off the room, the laughter, the words — the body building its own sensory bunker",
                "the chest caving inward, a physical implosion, the ribs contracting as if the body is trying to protect the heart by compressing it",
                "numbness spreading from the center outward — the emotional circuit breaker tripping, the feeling too large for the system to process at once",
                "the throat closing completely, not in the gradual tightening of sadness but in the sudden seal of someone whose right to speak has been revoked",
                "a coldness that begins at the extremities and creeps inward — the blood retreating from the surface, the body going into conservation mode",
                "social pain activating the same neural pathways as physical pain — the brain making no distinction between a blow to the body and a blow to the dignity",
                "the specific, bone-deep exhaustion that arrives after the initial shock passes — the body depleted by what it took to survive the moment",
            ]),
            ("Mental Responses", vec![
                "the mind going blank — not the empty calm of peace but the white static of overload, the processing power crashing",
                "the thought arriving with devastating clarity: everyone saw, and no one intervened",
                "dissociation — the self separating from the body, watching from above, from outside, from anywhere that isn't here",
                "the status claim collapsing: not just 'I failed' but 'I had no right to try' — the deepest cut of humiliation",
                "a rage building beneath the numbness — white-hot, directionless, searching for a target and finding only the self",
                "the preemptive obsession beginning: this moment replaying in every quiet hour, every sleepless night, for weeks, for months, for years",
                "the loss of behavioral capacity — actions that were possible before this moment now feel permanently beyond reach",
                "calculating who witnessed it, what they'll say, how far the story will spread — the mind mapping the damage radius",
                "the thought that this is now part of the story others tell about you, and you have no voice in the telling",
                "the terrible question: was the degrader right? the seed of doubt planted in the exact place the self-worth used to be",
            ]),
            ("Suppression Cues", vec![
                "forcing the head up through sheer will, the chin lifting against the gravity of the moment — defiance as a survival mechanism",
                "the mask assembling itself from nothing: neutral face, steady voice, unhurried movements — the performance of being unaffected that costs everything",
                "smiling — the specific, terrible smile of someone who will not give the degrader the satisfaction of seeing the wound",
                "speaking in a calm, measured tone that vibrates almost imperceptibly with what's being held back",
                "converting the humiliation to anger immediately, the pivot to fury a defense against the vulnerability of the wound",
                "laughing it off with a sound that isn't quite right — too sharp, too quick, the humor a bandage applied to a hemorrhage",
                "walking out with deliberate composure, the pace controlled, the back straight — the collapse saved for the car, the bathroom, the empty room on the other side of the door",
                "compartmentalizing the event — filing it away, sealing the room, functioning on the surface while the wound bleeds internally",
                "reframing it as a story of their cruelty rather than your vulnerability — 'that says everything about them and nothing about me' — the mantra repeated until the fists unclench",
                "the private reconstruction beginning immediately: rebuilding the self-image brick by brick, alone, in the dark, starting from whatever rubble remains",
            ]),
        ]),
        ("Regret", "The backward-looking wish that a different choice had been made", vec![
            ("Physical Signals", vec![
                "scrubbing a hand down the face slowly, the palm dragging from forehead to chin as if trying to wipe the memory off",
                "a hand pressing flat against the breastbone — the gesture of someone locating a pain that has no physical address",
                "the heavy, full-body sigh that empties the lungs and doesn't refill them completely, the breath trailing off into nothing",
                "staring at a specific spot — a chair, a doorway, a stretch of road — the eyes fixed on the exact place the other path diverged",
                "shaking the head in slow, small movements, the refusal arriving years too late to change anything",
                "holding an old photograph, an unsent letter, a kept ticket — artifacts from the version of life that didn't happen",
                "jaw working silently, the mouth rehearsing words that should have been said at a time when saying them mattered",
                "eyes closing and staying closed, the face flinching at something only memory can see",
                "pressing thumb and forefinger into the bridge of the nose, the headache of hindsight settling in",
                "pacing a room without purpose, the body enacting a search for something the mind knows can't be found",
                "picking up the phone and putting it down — the call that should have been made months, years, decades ago",
                "sitting in a car after arriving but not getting out, the engine off, the hands still on the wheel, the mind somewhere else entirely",
                "touching a scar, a ring, an empty space where something used to be — the body's map of choices and their consequences",
            ]),
            ("Internal Sensations", vec![
                "an ache behind the sternum that sharpens at specific triggers — a name, a date, a place — and dulls to a background hum the rest of the time",
                "the orbitofrontal cortex firing its counterfactual comparison: what happened versus what would have happened — the brain computing the gap between the two and registering the result as pain",
                "a heaviness settling in the chest that is not grief's hollowness but regret's density — the weight of a road not taken still occupying space",
                "the stomach turning at the memory, the nausea of self-knowledge arriving years after the fact",
                "a tightness in the throat when the subject surfaces — the lump composed of every word that should have been spoken and wasn't",
                "the body aging around the regret — the stiffness in the morning, the fatigue in the afternoon, the sleeplessness at night, all amplified by the carrying",
                "a phantom ache in the hands — the sensation of reaching for something that was once within grasp and is no longer there",
                "the specific exhaustion of fighting a battle that was lost in the past, the effort expended on terrain that can't be reclaimed",
                "warmth leaving the body when the regret surfaces, the temperature dropping as the mind travels backward to a colder version of the self",
                "the heart beating with a dull, heavy rhythm — not the quick pulse of anxiety but the slow thud of something permanent",
            ]),
            ("Mental Responses", vec![
                "the counterfactual loop: 'if I had said yes,' 'if I had stayed,' 'if I had called' — the branching paths replayed with obsessive specificity",
                "the orbitofrontal cortex running its comparison engine ceaselessly — computing what was chosen against what was available, finding the deficit, reporting it as anguish",
                "the awareness that the choice was made freely and cannot be blamed on anyone else — regret's particular cruelty: authorship",
                "wondering who you would be now if that one thing had gone differently — the ghost of the alternate self, more successful, more loved, more alive",
                "the deathbed inventory running early: Bronnie Ware's five regrets echoing — the life not lived true, the feelings not expressed, the friendships not tended",
                "the inability to let the alternate path go, even when the current path has good things in it — the mind always checking back",
                "the thought arriving with surgical precision: I knew better, and I chose wrong anyway",
                "replaying not just the moment but the moment before the moment — the last second when a different choice was still possible",
                "the terrible honesty of regret: unlike guilt, it doesn't always involve wrongdoing — sometimes the worst regrets are about what you simply didn't do",
                "the slow, dawning understanding that some choices are irreversible — the window is not just closed but walled over, the door removed from the frame",
            ]),
            ("Suppression Cues", vec![
                "the brisk 'no point dwelling on it' — the philosophy of forward motion deployed as a dam against the backward pull",
                "reframing the choice as the only option: 'I didn't really have a choice' — the revision that edits out the agency and with it the responsibility",
                "staying busy, staying distracted, staying in the present tense by force — because stillness is where the past lives",
                "telling the story with the regret edited out, the version where everything happened for a reason and the current life is the right one",
                "converting regret into wisdom: 'I learned from it' — which may be true but doesn't address the ache, only repackages it",
                "avoiding the place, the person, the song that triggers it — the geography of avoidance maintained without explanation",
                "dismissing the feeling as self-indulgent: 'everyone has regrets, get over it' — the internal voice performing toughness for an audience of one",
                "making the opposite choice now, aggressively — saying yes to everything, calling everyone back, overcompensating in the present for the deficit in the past",
                "telling someone 'I have no regrets' and meaning it as aspiration rather than truth",
                "the 3 a.m. negotiation with the self: acknowledging the regret exists, agreeing it cannot be fixed, and requesting — not commanding — that the mind let it rest until morning",
            ]),
        ]),
        // ── Surprise & Disorientation Family ──
        ("Surprise", "The sharp, neutral jolt of the unexpected — brief and quickly resolved into another emotion", vec![
            ("Physical Signals", vec![
                "eyebrows shooting upward, forehead creasing into horizontal lines — the face opening wide to take in more information",
                "eyes going round, the whites visible all around the iris, the lids pulled back as far as they'll go",
                "the mouth dropping open, jaw unhinging, the face forming the classic O that is surprise's universal signature",
                "the sharp intake of breath — a gasp that pulls air in and holds it, the lungs bracing",
                "the whole body freezing mid-action — a fork halfway to the mouth, a step incomplete, the hand suspended in air",
                "hands flying to the mouth, the chest, the sides of the head — the body's scatter-shot attempt to cover its own reaction",
                "stumbling backward a half-step, the body retreating from the unexpected before the mind has classified it",
                "the head jerking back on the neck, the chin pulling in — the orienting reflex redirecting the face toward the source",
                "blinking rapidly in the first seconds, the eyelids cycling as the visual system recalibrates",
                "the phone, the glass, the book slipping from suddenly nerveless fingers",
                "spinning toward the source — the whole body pivoting, feet replanting, the posture reorganizing around the new information",
                "a hand reaching out to grip the nearest surface — a table, a doorframe, another person — the body stabilizing against the jolt",
                "the voice emerging as a yelp, a squeak, a single explosive syllable — the sound escaping before words can form",
                "the face cycling through surprise and into its successor emotion in real time — the O becoming a grin, a grimace, a blank stare, depending on what the surprise reveals",
            ]),
            ("Internal Sensations", vec![
                "a jolt of adrenaline detonating in the center of the chest — the full sympathetic spike arriving in under a second",
                "the heart lurching, skipping a beat, then slamming back with a heavy double-thump",
                "the stomach swooping, the brief freefall sensation of the ground shifting under established expectations",
                "a tingling rush flooding the extremities — the blood redirecting, the hands going electric, the scalp prickling",
                "the lungs seizing on a held breath, the diaphragm locked, the body waiting for the signal to resume",
                "time stretching in the gap between the event and the comprehension — the moment dilating, each micro-second distinct",
                "the amygdala firing before the prefrontal cortex can respond — the body reacting to the unexpected a full beat before the mind understands it",
                "a wave of heat or cold washing through, the temperature swinging in whichever direction the surprise will resolve — warm for joy, cold for dread",
                "the muscles tensing simultaneously, the whole body going taut like a plucked string, vibrating with readiness for whatever comes next",
                "the brief, vertiginous blankness — a gap in the internal monologue, the mind rebooting from one reality to another",
            ]),
            ("Mental Responses", vec![
                "the cognitive blank — a white flash of zero thought, the mind wiped clean by the unexpected for one pure second",
                "the frantic reorientation: what just happened, is this real, did I see that correctly",
                "reality realigning — the mental model of the world cracking along a new fault line and rebuilding around the new information",
                "the rapid emotional sort: is this good news or bad news — the answer determining which emotion surprise will hand the baton to",
                "the replay starting immediately, the mind looping back to the instant before the surprise to verify the sequence",
                "words failing — the vocabulary temporarily offline, the mouth opening and closing without producing anything coherent",
                "the reflexive question: 'wait — what?' — spoken aloud or thought, the mind's demand for a second take",
                "disbelief arriving as a buffer — 'this can't be right' — the mind buying time before accepting the new reality",
                "the awareness that everything before this moment was based on assumptions that just proved wrong",
                "the brief, intoxicating freedom of not knowing what happens next — surprise as the only moment when the future is genuinely open",
            ]),
            ("Suppression Cues", vec![
                "catching the gasp before it escapes, converting it to a controlled exhale through the nose",
                "the eyebrows lifting then being pulled back down by conscious effort — the face disciplined before the full expression can form",
                "pressing the lips together hard over the O, the mouth sealed against the sound it wants to make",
                "the quick blink-and-nod — acknowledging the information with studied calm, as if none of this is news",
                "reaching for the glass, the pen, the phone — giving the hands an occupation to mask the tremor of the jolt",
                "speaking first: 'I know' or 'I figured' — the pre-emptive claim to unsurprise, the composure performed retroactively",
                "folding the reaction into humor: 'well, didn't see that coming' — the joke arriving faster than the authentic response",
                "the delayed reaction deployed strategically — absorbing the information with a poker face, then responding at a pace that suggests control",
                "channeling the jolt into a single raised eyebrow rather than the full-face display — the minimalist version that admits nothing",
                "turning away for one second — the time it takes to compose the face — then turning back with an expression that reveals only what was chosen",
            ]),
        ]),
        ("Shock", "Surprise at extreme intensity, often paralyzing — the system overwhelmed before it can process", vec![
            ("Physical Signals", vec![
                "the body going completely rigid — every muscle locking at once, the posture frozen mid-whatever-it-was-doing",
                "the color draining from the face in real time, the skin going the pale gray of wet cement",
                "eyes staring without blinking, fixed and glassy, the pupils dilated but the gaze registering nothing",
                "the mouth moving without sound — lips shaping words that don't arrive, the connection between brain and voice severed",
                "the phone, the cup, the keys falling from suddenly boneless fingers, the crash of the dropped object the loudest sound in the room",
                "swaying on the feet, the body losing its sense of vertical, a hand reaching out blindly for something solid",
                "sitting down hard — not choosing to sit but the legs simply giving way, the body folding onto whatever surface is nearest",
                "hands pressed against the sides of the head, the fingers gripping the hair, the posture of a mind trying to hold itself together",
                "the voice, when it finally comes, arriving from very far away — thin, flat, affectless, as if being transmitted from another room",
                "movements becoming slow, underwater, dreamlike — reaching for a glass takes five seconds, standing from a chair takes ten",
                "repeating the same phrase on a loop: 'no,' or 'what,' or 'I don't understand' — the needle stuck in the groove",
                "the wandering gaze that can't settle — eyes moving from object to object without focusing, the visual system searching for something the mind can process",
                "hyperventilating — the breath coming in short, rapid, shallow gulps, the body flooding itself with oxygen it can't use",
                "vomiting or retching, the body ejecting what the mind cannot — the gut's response to information too extreme for the higher systems",
            ]),
            ("Internal Sensations", vec![
                "the dorsal vagal shutdown — heart rate plummeting, blood pressure dropping, the parasympathetic system pulling the emergency brake",
                "numbness spreading from the core outward like a spill, the feeling disappearing from the extremities first, then the center",
                "a ringing in the ears that rises until it obliterates all other sound — the auditory system closing like a shutter",
                "the body going cold from the inside, a chill that isn't temperature but the withdrawal of the nervous system from the surface",
                "time fracturing — not slowing but breaking into discontinuous frames, the continuity of experience interrupted",
                "the sensation of falling while standing still, the vestibular system reporting that the ground has moved",
                "endogenous opioids flooding the system — the body's own painkillers deploying, numbing the emotional and physical simultaneously",
                "the feeling of watching oneself from outside — depersonalization arriving as the mind's emergency exit from a reality it can't inhabit",
                "breathing becoming a conscious labor, each inhale requiring a decision the autonomic system is no longer making on its own",
                "the stomach turning to ice, the gut clenching into a fist, the body bracing at the visceral level for what the mind hasn't finished processing",
            ]),
            ("Mental Responses", vec![
                "the cognitive whiteout — not a blank but a blizzard, too much information arriving too fast for any of it to resolve into a thought",
                "reality refusing to cohere — the words heard, the scene witnessed, but the meaning sliding off, unable to attach itself to anything that was true five seconds ago",
                "the mind looping on the last normal moment: 'I was just making coffee, I was just driving, I was just—' the before used as an anchor against the after",
                "dissociation — the self separating from the body, from the room, from the event, the consciousness floating somewhere above and to the left",
                "the derealization arriving: 'this isn't real, this is a dream, I'm going to wake up' — the mind rejecting the input rather than processing it",
                "thoughts arriving in fragments that don't connect — no narrative, no sequence, just shards: a face, a sound, a color, a word",
                "the inability to answer simple questions: 'are you okay?' producing only a stare, the interrogative not reaching the language center",
                "time losing its sequence — not knowing if the event happened seconds or minutes ago, the chronology scrambled",
                "the mind oscillating between total blankness and hyper-vivid recall of a single irrelevant detail: the pattern on the wallpaper, the number on a license plate, the song playing on the radio",
                "the delayed processing — the full emotional impact arriving hours, days, or weeks later, when the system has finally rebooted enough to feel what it couldn't feel at the time",
            ]),
            ("Suppression Cues", vec![
                "snapping into action mode — the crisis manager emerging, the voice steady and commanding while the self is somewhere far away",
                "speaking in short, precise, operational sentences: 'call 911, don't move, where does it hurt' — logistics as a shield against the abyss",
                "gripping something hard — a steering wheel, a counter edge, a hand — the physical anchor holding the self in the present",
                "forcing the breathing into a rhythm: in-two-three, out-two-three — the manual override of a system that has crashed",
                "the mask going on instantly, the composure assembled from muscle memory rather than actual calm",
                "performing normalcy for someone else's sake — a child, a patient, a crowd — the suppression motivated by the need to not make this worse",
                "the clipped 'I'm fine' that is neither a lie nor the truth but a placeholder for the processing that hasn't begun",
                "compartmentalizing in real time — boxing the event, sealing it, functioning on the surface while the deeper systems are offline",
                "making a list, following a protocol, doing the next correct thing — the mind latching onto procedure because improvisation is impossible",
                "the delayed collapse: functioning perfectly for hours, days, the performance impeccable — then the trembling beginning in a quiet room, alone, the shock finally arriving to claim what it's owed",
            ]),
        ]),
        ("Confusion", "The unsettled, searching discomfort of not understanding", vec![
            ("Physical Signals", vec![
                "the brow furrowing deeply, the skin between the eyebrows pinching into vertical lines — the face of a mind working hard and finding nothing",
                "head tilting to one side, the posture of a dog hearing a frequency it can't identify",
                "squinting, the eyes narrowing as if the problem were a distant sign the body could read by getting optically closer",
                "scratching the back of the head, the hand moving in slow circles — the ancient self-soothing gesture of a brain under load",
                "mouth opening slightly, then closing, then opening again — the speech center drafting and discarding attempts to articulate the gap",
                "looking from person to person in a group, searching each face for the comprehension missing from one's own",
                "hands gesturing vaguely in the air, the fingers tracing shapes that don't resolve — the body trying to model what the mind can't",
                "re-reading the same line, the same email, the same sign — the eyes repeating the scan as if the words will rearrange themselves into sense",
                "blinking rapidly, the eyelids cycling at a rate that suggests the visual system is recalibrating",
                "rubbing the temples or the bridge of the nose, the hands addressing the headache of cognitive overload",
                "standing in the middle of a room having forgotten why the body walked there, the feet paused between two intentions",
                "turning in a slow, uncertain circle — in a parking lot, a hallway, a new city — the body searching for a landmark the mind can't supply",
                "the stuttering start to a question: 'wait — so — but — how does that—' the sentence unable to find its structure",
                "picking up an object, putting it down, picking up a different object — the hands enacting the mind's inability to settle on a course",
            ]),
            ("Internal Sensations", vec![
                "a fog in the skull — not pain but thickness, the thinking slowed as if the thoughts are moving through gauze",
                "the prefrontal cortex straining under load — a physical sensation of effort behind the forehead, the brain's equivalent of a muscle trembling under a weight too heavy to hold",
                "pupils dilating as cognitive load increases — the eyes opening wider to take in more data, the body's attempt to solve the problem by seeing more",
                "a low-grade anxiety humming beneath the confusion — the nervous system registering that not-understanding is a vulnerability",
                "the stomach tightening slightly, the gut's response to uncertainty — not nausea but unease, the digestive system echoing the cognitive distress",
                "a restless, itching discomfort that isn't located anywhere specific — the whole body unsettled by the mind's inability to resolve",
                "the head feeling heavy, the neck straining to hold it, the physical weight of a problem that won't solve",
                "a buzzing blankness behind the eyes — the working memory at capacity, new information arriving but finding no place to land",
                "the specific frustration of almost-understanding — the answer flickering at the edge of comprehension and not quite arriving",
                "exhaustion arriving disproportionately fast — the metabolic cost of confusion, the brain burning glucose at an unsustainable rate",
            ]),
            ("Mental Responses", vec![
                "the mind reaching for a pattern and closing on empty air — the category that should hold this information doesn't exist yet",
                "thoughts starting and stopping, each thread picked up and dropped before it reaches its end",
                "the reflexive replay: going back to the last thing that made sense and trying to build forward from there",
                "a growing frustration at the self — 'why can't I understand this' — the confusion generating its own secondary emotion",
                "the mental model of the situation cracking, the framework that was holding the facts together no longer fitting",
                "questioning whether the problem is in the information or in the self: is this genuinely unclear, or am I not smart enough",
                "the desperate simplification — trying to reduce the complexity to one question, one variable, one thing that might unlock the rest",
                "the mind toggling between competing interpretations, unable to commit to either, each one undermining the other",
                "the awareness that asking for help would resolve this in seconds, weighed against the reluctance to admit the not-knowing",
                "the thought looping without progress: a hamster wheel of the same incomplete reasoning, the same dead-end, the same 'but then how—'",
            ]),
            ("Suppression Cues", vec![
                "nodding along as if the explanation is landing, the face performing comprehension while the mind races to catch up",
                "saying 'right, right' or 'makes sense' at intervals — the verbal filler of someone who is lost and doesn't want to admit it",
                "writing notes furiously, the pen moving as a cover for the fact that the content isn't being understood in real time",
                "laughing and saying 'I'm terrible with this stuff' — converting confusion into self-deprecation to lower the stakes",
                "deferring: 'I'll look at this later' — buying time, privatizing the confusion, moving it to a setting where not-knowing isn't visible",
                "asking a tangential question to demonstrate engagement without revealing that the core point was missed entirely",
                "smiling through it and planning to google everything the moment the conversation ends",
                "staying silent rather than asking the question that would expose the gap — the cost of confusion in public",
                "repeating back what was said in slightly different words, hoping the paraphrase will trigger the understanding the original didn't",
                "the quiet relief of someone else asking the same question first — the exhale of shared confusion, the permission to not-know granted by company",
            ]),
        ]),
        ("Disbelief", "The refusal or inability to accept a fact as real — reality and expectation in collision", vec![
            ("Physical Signals", vec![
                "the slow head shake — not a single emphatic no but a continuous, almost involuntary rotation, the body rejecting what the ears just received",
                "the double-take — the eyes snapping back to the source, the head whipping around for a second look, the body demanding a recount",
                "rubbing the eyes as if the problem might be visual, as if the scene could be cleared like a smudge on a lens",
                "the mouth forming words that don't arrive — 'that's not — you can't — how is that—' each attempt collapsing before completion",
                "leaning back, the body physically retreating from the information, putting distance between the self and the new reality",
                "a short, incredulous laugh — the sound of the nervous system defaulting to humor when no other response will fit",
                "looking to other faces in the room for confirmation: did you hear that too, is this really happening",
                "picking up the letter, the phone, the document and re-reading it from the beginning, as if the words might rearrange themselves on the second pass",
                "pressing a hand flat against the forehead, the fingers splayed, the gesture of a mind trying to physically hold its model of reality together",
                "the jaw loosening, the mouth falling open and staying there — not surprise's brief O but the sustained gape of something that refuses to compute",
                "turning away and turning back, the body caught between wanting to escape the information and needing to verify it",
                "asking 'what?' or 'say that again' not because the words weren't heard but because hearing them again might change what they mean",
                "scoffing — the sharp exhale through the nose, the sound of the mind's first defense: dismissal",
            ]),
            ("Internal Sensations", vec![
                "cognitive dissonance firing — the anterior cingulate cortex activating, the brain registering the collision between what was believed and what is being reported",
                "a lurching sensation in the stomach, the gut's response to the ground shifting under an assumption that was load-bearing",
                "a strange lightness in the head, the faint vertigo of a worldview tilting on its axis",
                "the chest tightening with the effort of holding two contradictory realities simultaneously — what was true and what is apparently true now",
                "a tingling numbness in the hands and face, the blood retreating from the periphery as the brain redirects resources to the crisis of comprehension",
                "the heart rate climbing not from fear but from the sheer computational effort of trying to reconcile the irreconcilable",
                "a flush of heat or a wash of cold — the autonomic nervous system spiking in whatever direction the disbelief leans: outrage or dread",
                "the specific sensation of reality delaminating — a peeling-apart feeling, as if the world has split into the version that existed before this information and the version that exists now",
                "the mouth going dry, the throat clicking on a swallow — the body's alarm that something fundamental has shifted",
                "a buzzing static behind the ears, the brain's white noise generator running at full capacity to buffer the incoming data",
            ]),
            ("Mental Responses", vec![
                "the mind running the information against every known fact and finding it doesn't fit — not a single slot in the existing framework that will hold it",
                "the reflexive search for an alternative explanation: they're lying, this is a mistake, there's been a misunderstanding, this is a joke",
                "reality testing at full speed — checking the source, checking the evidence, checking one's own perception: am I awake, am I sober, did I hear correctly",
                "the belief perseverance kicking in: the existing model fighting to survive, the mind marshalling every reason the new information must be wrong",
                "the thought arriving as a demand rather than a statement: 'prove it' — the mind requiring evidence proportional to the claim",
                "oscillating between acceptance and rejection, the mind toggling like a switch that can't commit to either position",
                "the world reorganizing in the background — even as the conscious mind protests, some deeper system is already quietly rebuilding the model around the new information",
                "the specific cognitive pain of letting go of something that was believed deeply and turns out to be false",
                "a fierce, protective anger at whoever delivered the information, as if shooting the messenger could un-ring the bell",
                "the delayed acceptance arriving not as a decision but as an exhaustion — the mind finally too tired to maintain the denial, the new reality settling in like sediment",
            ]),
            ("Suppression Cues", vec![
                "the quick nod and the composed 'I see' — the exterior accepting what the interior is still rejecting",
                "moving immediately to practical matters: 'okay, so what do we do now' — the response of someone who will process the disbelief later, in private",
                "keeping the face neutral through sheer discipline while the mind is screaming 'this can't be right'",
                "asking detailed, clinical questions — dates, numbers, specifics — channeling the disbelief into interrogation",
                "making a joke: 'you're kidding, right?' delivered with a laugh that is testing whether this might actually be a joke",
                "saying 'I need a minute' and leaving the room — the physical separation buying time for the internal recalibration",
                "writing it down, the act of putting pen to paper making the information real in a way that hearing it didn't",
                "calling someone else to verify — needing a second source before the belief system will consent to update",
                "the careful control of the face and voice that allows exactly one crack: the eyebrow that rises, the pause that stretches a beat too long",
                "accepting it outwardly and immediately, performing adjustment — while internally the disbelief will take days, weeks, to fully resolve",
            ]),
        ]),
        // ── Disgust Family ──
        ("Disgust", "The visceral recoil from something contaminating, revolting, or morally corrupt", vec![
            ("Physical Signals", vec![
                "the upper lip curling back from the teeth, the nose wrinkling hard enough to crease the bridge — the ancient gape reflex, the body's first draft of a retch",
                "the head snapping away, chin tucking toward the shoulder, as if the neck has its own opinion and it's no",
                "a hand rising to cover the nose and mouth, fingers pressing hard, trying to seal off the entry points",
                "the whole body recoiling — a step back, shoulders pulling in, center of gravity shifting away from the source",
                "tongue pressing forward against the backs of the teeth or protruding slightly, the oral expulsion reflex activating before the brain can override it",
                "eyes narrowing to slits, the lower lids pushing up, reducing the visual field as if seeing less of it might make it less real",
                "the dry heave — a sudden, convulsive contraction of the abdomen, the diaphragm lurching upward, producing nothing but the sound of refusal",
                "nostrils clenching shut, the breath routing through the mouth in short, shallow pulls to bypass the olfactory system entirely",
                "pushing a plate, an object, a document away with the fingertips, touching as little surface area as possible — contamination logic in the fine motor system",
                "wiping the hands on clothing repeatedly even when nothing was touched, the palms rubbing against thighs as if cleaning off something invisible",
                "the face cycling through micro-expressions too fast to mask — the wrinkled nose, the bared teeth, the squinted eyes — before settling into a controlled grimace",
                "spitting or the visible effort of not spitting, the mouth flooding with saliva as the body prepares to expel",
                "holding the contaminated hand away from the body, fingers splayed, as if the hand itself has become the offending object",
                "the shoulders hunching up around the ears in a full-body cringe, the posture of someone trying to occupy as little space near the source as possible",
                "turning a child's face away, shielding another person's eyes — the protective instinct arriving faster than the verbal warning",
            ]),
            ("Internal Sensations", vec![
                "the stomach lurching upward in a slow, rolling wave, not the sharp drop of fear but an ascending nausea that climbs toward the throat",
                "saliva flooding the mouth — the body's pre-vomiting preparation, the salivary glands activating to protect tooth enamel from what's about to come up",
                "the vagus nerve firing, heart rate dropping rather than climbing, blood pressure falling — disgust's parasympathetic signature, the opposite of fear's alarm",
                "a crawling sensation across the skin, as if whatever was seen or smelled has already made contact and is spreading",
                "the throat constricting in rhythmic spasms, the gag reflex hovering at the edge of activation, swallowing fighting against the body's urge to expel",
                "a greasy, heavy wrongness settling in the pit of the stomach, the gut's verdict delivered before the brain has finished processing",
                "the insula lighting up — the same brain region that processes rotten tastes now firing at a moral violation, the body unable to distinguish between types of poison",
                "a prickling heat across the face and neck that isn't a blush but an immune response, the skin flushing as if fighting off contact with a pathogen",
                "the specific sensation of contamination — a phantom residue on the hands, in the mouth, across the skin — that no amount of washing will immediately relieve",
                "a tightening across the soft palate, the back of the throat closing like a valve, the body sealing itself against ingestion",
                "appetite vanishing instantly and completely, the thought of food becoming its own source of revulsion, the digestive system shutting down",
            ]),
            ("Mental Responses", vec![
                "the categorical judgment arriving whole and immediate: wrong, unclean, do not touch — no deliberation, no nuance, just the verdict",
                "contamination logic cascading outward — the source is tainted, therefore everything it touched is tainted, therefore everything near it is suspect",
                "the desperate need to look away warring with the inability to stop looking, the eyes drawn back to the source like a tongue to a broken tooth",
                "moral disgust borrowing the vocabulary of physical disgust: filthy, rotten, toxic, sick — the mind reaching for the body's oldest rejection language",
                "the immediate, involuntary ranking: this is beneath me, beneath us, beneath the species — disgust's hidden architecture of hierarchy",
                "the urge to clean, to purify, to restore order — scrubbing hands, airing rooms, burning evidence — as if the contamination can be physically undone",
                "a sharp, clarifying anger arriving on disgust's heels — the outrage that this thing exists, that someone did this, that it was allowed",
                "the mind rewriting proximity as complicity: I was near it, I saw it, does that make me part of it — the guilt of the witness",
                "the reflexive search for someone to blame, to punish, to hold responsible for the violation — disgust demanding accountability",
                "a sudden, fierce protectiveness over anything innocent or clean nearby — children, food, open wounds — the guardian instinct triggered by the presence of contamination",
                "the thought arriving unbidden and unwelcome that you will remember this, that the image is already being etched into long-term storage against your will",
            ]),
            ("Suppression Cues", vec![
                "swallowing hard and repeatedly, forcing the gag reflex back down through sheer muscular effort, the throat bobbing visibly",
                "breathing through the mouth in controlled, measured pulls, routing air past the tongue to bypass the nose entirely",
                "fixing the face into neutral through conscious effort — relaxing the upper lip, un-wrinkling the nose, one muscle group at a time",
                "converting the recoil into a casual step back, disguising the retreat as a shift in weight or a reach for something behind",
                "the polite smile held in place like a shield while the eyes remain flat and the nostrils stay pinched",
                "making a quiet excuse to leave — needing air, needing the bathroom, needing to check something — the exit manufactured to look voluntary",
                "focusing intently on something neutral: a wall, a window, a point in the middle distance — anything that isn't the source",
                "channeling the disgust into clinical language: 'that's concerning' or 'that's not ideal' — the euphemism doing the work the face is forbidden to do",
                "taking a sip of water, the swallow resetting the throat, the glass providing something for the hands and mouth to do besides recoil",
                "the single, controlled exhale through the nose — not quite a snort, not quite a sigh — the only crack in the composure, released like a pressure valve",
            ]),
        ]),
        // ── Anticipation & Motivation Family ──
        ("Hope", "The forward-leaning belief that a desired outcome is possible", vec![
            ("Physical Signals", vec![
                "the body orienting toward the source — a door, a phone, a horizon — the torso angling forward as if leaning into a wind that hasn't arrived yet",
                "eyes lifting and holding on something in the middle distance, the gaze soft but fixed, already watching for what might come",
                "a slow, unguarded inhale — the chest expanding fully, the shoulders dropping, the ribcage opening as if making room for the possibility",
                "hands unclenching without the person noticing, the fingers loosening from fists they didn't know they were making",
                "the chin lifting slightly, not in defiance but in readiness, the neck straightening from whatever slump preceded this moment",
                "a tentative smile that starts at the corners of the mouth and stalls there, not yet willing to commit to the full expression",
                "sitting forward on the edge of a chair, weight shifted to the balls of the feet, the body halfway to standing without having decided to rise",
                "touching a talisman — a ring, a locket, a letter — the fingers finding it automatically, drawing reassurance from something tangible",
                "the voice lifting in pitch at the end of statements, turning facts into half-questions, as if checking whether the universe agrees",
                "eyes brightening with a faint gloss — not tears but the shimmer of emotion rising to the surface without quite breaking through",
                "clasping hands together in the lap or against the chest, the fingers interlocking, holding the feeling in place as if it might escape",
                "glancing at someone else to see if they feel it too — the quick, searching look that asks 'is this real, is this happening'",
                "the restless energy of someone who wants to act but has nothing to do yet — straightening a collar, smoothing a surface, small preparations for a future that may arrive",
                "standing at a window, a hand resting on the glass, the posture of someone keeping vigil without wanting to call it that",
            ]),
            ("Internal Sensations", vec![
                "a warmth kindling behind the sternum — not the blaze of joy but a low, cautious flame, cupped and protected",
                "dopamine beginning its slow drip in the reward circuits, the nucleus accumbens activating not at pleasure but at the possibility of pleasure",
                "the chest feeling lighter, as if a weight that had been sitting there for days has shifted — not lifted, but shifted",
                "a fluttering in the stomach that could be mistaken for anxiety except it pulls upward instead of sinking down",
                "the throat loosening, the lump that lived there thinning, swallowing becoming easier for the first time in hours",
                "a tingling alertness spreading through the limbs, the body waking from the low-power mode of resignation",
                "the heart beating with a steadier, more purposeful rhythm — not faster but fuller, each beat carrying more conviction",
                "a prickling behind the eyes, the tear ducts responding to the specific pain of wanting something badly enough to be hurt by its absence",
                "the muscles in the shoulders and jaw releasing tension they'd been holding so long the relaxation itself registers as a sensation",
                "a buoyancy in the core, a rising feeling, as if the diaphragm has remembered how to float instead of brace",
            ]),
            ("Mental Responses", vec![
                "the mind constructing the scene — what it will look like when the good thing happens, where everyone will be standing, what will be said first",
                "the cautious internal arithmetic: the odds, the evidence for and against, the mind building a case it desperately wants to win",
                "the future tense returning to the vocabulary — 'when' replacing 'if,' 'soon' edging out 'never,' the grammar of possibility reasserting itself",
                "the inner voice catching itself mid-plan and pulling back: don't get ahead of yourself, don't jinx it, be careful",
                "memories of past disappointments surfacing uninvited, the mind's immune system trying to inoculate against another letdown",
                "the realization that you've been holding your breath metaphorically for weeks and something has just given permission to exhale",
                "a fragile clarity — seeing the path forward for the first time, even if it's narrow, even if it's uncertain",
                "the bargaining that hope generates: I'll do anything, I'll change, I'll be better — promises made to no one in particular",
                "a fierce protectiveness over the feeling itself, the awareness that sharing it might invite someone to question it",
                "the oscillation between believing and bracing — the mind toggling between 'this could work' and 'but what if it doesn't' in a rhythm that never quite resolves",
            ]),
            ("Suppression Cues", vec![
                "the deliberate flattening of the voice when discussing the possibility, keeping the tone neutral, clinical, as if it doesn't matter either way",
                "qualifying every hopeful statement with a hedge: 'we'll see,' 'I'm not counting on it,' 'it probably won't happen, but'",
                "refusing to say the hoped-for thing out loud, as if naming it might scare it away — superstition dressed as pragmatism",
                "changing the subject when someone asks directly, deflecting with a shrug or a joke to avoid exposing the tender thing underneath",
                "the studied indifference — leaning back instead of forward, arms crossed, face composed — all the body language of someone who hasn't already imagined the best-case scenario a hundred times",
                "preparing for the worst out loud while hoping for the best in silence — packing a bag for a trip you're telling everyone you probably won't take",
                "dismissing compliments or encouragement with 'don't' or 'stop' — not rudeness but the fear that someone else's hope will make yours harder to survive losing",
                "busying the hands with something practical to keep them from clasping, reaching, praying — the physical vocabulary of wanting",
                "the micro-flinch when someone says it's going to be fine — the face briefly showing how much rides on that being true before the mask returns",
                "laughing it off, making the hope smaller with humor: 'wouldn't that be something' delivered with a casualness that costs everything to maintain",
            ]),
        ]),
        ("Anticipation", "Expectant readiness for what is coming — can lean toward excitement or dread", vec![
            ("Physical Signals", vec![
                "the body going still except for one restless part — a bouncing knee, a tapping finger, a heel lifting and dropping — the nervous system leaking energy through a single outlet",
                "eyes fixed on the door, the clock, the phone, checking and rechecking at intervals that shorten as the moment approaches",
                "leaning forward in the seat, weight on the front edge, the body already aimed at what's coming like a sprinter in the blocks",
                "pupils dilating, the eyes widening slightly, taking in more light, more information, the visual system upgrading to high alert",
                "hands finding each other — fingers interlacing, thumbs circling, the palms pressing together and releasing, a self-soothing rhythm",
                "the breath catching and holding for a beat too long before releasing in a controlled stream through the nose",
                "straightening up, pulling the shoulders back, the posture sharpening as if standing at attention for an inspection that hasn't started",
                "pacing with a purpose that dissolves at each wall — three steps toward the window, a pause, three steps back, a pause, the body caught in a holding pattern",
                "biting the inside of the cheek or the edge of a thumbnail, the mouth needing something to work on while the brain waits",
                "the quick, involuntary smile that surfaces and is pulled back — the face rehearsing a reaction it hasn't earned yet",
                "touching the face repeatedly — rubbing the chin, pressing the lips, brushing the hair back — the hands conducting their own nervous orchestra",
                "the voice going tight and bright, words coming faster, sentences clipped shorter, the speech pattern accelerating ahead of the event",
                "scanning a room or a crowd with quick, systematic sweeps, cataloging entrances, faces, changes — the surveillance instinct running on high",
                "shifting weight from foot to foot while standing, the body unable to commit to stillness, rocking in a slow metronome",
            ]),
            ("Internal Sensations", vec![
                "a coiled tension in the solar plexus — not pain but a wound-spring tightness, energy stored and waiting for the signal to release",
                "norepinephrine sharpening the senses, sounds becoming crisper, colors brighter, the edges of objects more defined — the world in sudden high resolution",
                "the heart rate climbing by degrees, not the spike of fear but a steady upward ramp, the cardiovascular system shifting into a higher gear",
                "a buzzing electricity under the skin, concentrated in the hands and the soles of the feet, the body's readiness pooling at its contact points",
                "the stomach fluttering in a way that could be excitement or nausea — the body genuinely unsure which direction this is going",
                "dry mouth arriving not from fear but from the sympathetic nervous system quietly diverting resources away from digestion toward alertness",
                "a restless heat building in the chest, not the warmth of emotion but the metabolic hum of a system preparing to act",
                "the jaw tightening and releasing in cycles, the masseter muscles clenching on a phantom bit, the body bracing in micro-intervals",
                "a heightened awareness of time — each second registering individually, the gap between now and the event stretching and compressing unpredictably",
                "the skin prickling at the back of the neck, the fine hairs lifting, the ancient perimeter alarm activating for something that isn't danger but demands attention anyway",
            ]),
            ("Mental Responses", vec![
                "the mind running scenarios in parallel — best case, worst case, most likely case — each one vivid and detailed, each one discarded and rebuilt",
                "time distortion: the last five minutes before the event expanding to fill an hour, every second accounted for and counted down",
                "the attention narrowing to a spotlight, peripheral concerns dimming to gray, the approaching event consuming all available bandwidth",
                "a rehearsal loop running beneath conscious thought — what to say, where to stand, how to react — the mind blocking scenes for a play that hasn't started",
                "the inability to read, to focus on a conversation, to complete a sentence — the cognitive system fully allocated to monitoring the approaching threshold",
                "a hyperawareness of cause and effect: every sound is a footstep, every notification is the one, every change in the environment is a signal",
                "the odd, doubled awareness of being both in this moment and already in the next — the mind occupying two timelines at once",
                "bargaining with the clock: if I don't look, time will move faster — if I stay busy, it will arrive sooner — the magical thinking of the impatient",
                "the creeping suspicion that you're not ready, arriving precisely when it's too late to prepare further",
                "a sharpened memory for details — the brain recording the minutes before the event with unusual fidelity, as if this waiting matters as much as what follows",
            ]),
            ("Suppression Cues", vec![
                "forcing the body into a practiced stillness — legs uncrossed, hands flat, breathing measured — a deliberate performance of calm over the buzzing underneath",
                "making small talk about nothing, filling the silence with words that require no thought, the conversation a screen for the countdown happening behind it",
                "checking the phone and putting it away with studied indifference, as if the notification being waited for isn't the only one that matters",
                "affecting boredom — a yawn, a stretch, a glance at the ceiling — the body language of someone who is definitely not counting the seconds",
                "channeling the restless energy into something productive: cleaning, organizing, answering emails — the busy hands covering for the racing mind",
                "keeping the voice at a deliberate, even pace, each word measured out to prevent the acceleration that would betray the internal tempo",
                "the single deep breath taken when no one is watching — the one honest exhale in an hour of performed composure",
                "arriving early but pretending not to have, checking the time once and pocketing the phone as if the timing were coincidental",
                "responding to 'are you nervous?' with a too-quick 'no' and an immediate pivot to logistics — when does it start, where should we sit, is there parking",
                "the controlled sip of water, the deliberate setting down of the glass, the careful management of every gesture to project a calm the body does not feel",
            ]),
        ]),
        ("Determination", "The locked-in, forward-driving resolve to see something through regardless of obstacles", vec![
            ("Physical Signals", vec![
                "the jaw setting hard, the masseter muscles bunching visibly at the hinge, teeth locked together — the face of someone who has stopped negotiating with the obstacle",
                "eyes narrowing, the lids lowering not in fatigue but in focus, the gaze sharpening to a point and holding there without blinking",
                "shoulders squaring, the spine straightening by degrees, the posture becoming a vertical line — the body's way of saying 'I'm not leaving'",
                "hands closing into fists at the sides, not in anger but in readiness, the fingers curling around a decision already made",
                "a single, decisive nod — to no one, to oneself — the head dipping once and locking forward, the internal contract now ratified",
                "rolling up sleeves, literally or figuratively — the small preparatory gesture that converts intention into action",
                "the chin dropping slightly, the head tilting forward, the posture of someone about to push through a door that hasn't opened yet",
                "planting the feet wider, distributing weight evenly, lowering the center of gravity — the stance of someone bracing to hold ground",
                "wiping palms on thighs once, sharply, the gesture clearing the slate, preparing the hands for whatever comes next",
                "the breath deepening and slowing deliberately, each exhale longer than the last, the respiratory system being brought under conscious command",
                "leaning into the work — shoulders forward, elbows on the table, head down, the body curved around the task like a shield",
                "the voice dropping in pitch and slowing in pace, each word placed with the weight of a stone being set in a wall",
                "pushing hair back from the face with a sharp, impatient sweep — clearing the field of vision, removing any obstruction between the self and the target",
                "the steady, metronome rhythm of someone who has found their pace and will not be moved from it — footsteps, keystrokes, breaths, all even and relentless",
            ]),
            ("Internal Sensations", vec![
                "a tightening in the core, the abdominal muscles engaging as if bracing for impact — the body fortifying its center before the push",
                "the prefrontal cortex burning through glucose at an accelerated rate, the specific metabolic heat of sustained willpower — a warmth behind the forehead that isn't fever",
                "the anterior mid-cingulate cortex firing, consolidating competing signals into a single directive: continue — the neural hub of resolve overriding the body's complaints",
                "a narrowing of sensation, peripheral discomfort dimming as the nervous system triages — sore muscles, hunger, fatigue all deprioritized in favor of the task",
                "the heart beating with a steady, workmanlike rhythm — not the sprint of fear or excitement but the marathon pace of endurance",
                "adrenaline running at a low, sustained simmer rather than a spike — enough to sharpen but not enough to shake, the chemistry of the long haul",
                "muscles aching from sustained tension but the pain registering as information rather than a command to stop — the body's protests noted and overruled",
                "a locked-in feeling behind the eyes, the visual system tunneling, the world narrowing to the task and its immediate inputs",
                "the jaw throbbing from being clenched too long, the ache arriving as proof of how hard the resolve has been held",
                "a deep, almost gravitational pull forward — not excitement's lift but determination's drag, the body leaning into resistance like a plow into soil",
            ]),
            ("Mental Responses", vec![
                "the internal voice simplifying to imperatives: keep going, don't stop, one more, again — the vocabulary of determination stripped to its studs",
                "the obstacle reframing in real time — not a wall but a problem, not a problem but a sequence of steps, the mind converting impossibility into logistics",
                "a deliberate refusal to calculate the remaining distance, the mind forbidding itself from asking 'how much further' because the answer doesn't matter",
                "the future collapsing to a single point: the next action, the next word, the next step — strategic vision narrowing to tactical execution",
                "the quiet, cold acknowledgment that this will hurt and that the hurt is acceptable — the internal negotiation completed, the price agreed upon",
                "every voice that says 'stop' being met with the same flat, automatic response: no — not argued with, not considered, just overruled",
                "the memory of why this matters rising to the surface unbidden — the face, the promise, the reason — and being held there like a torch in a dark corridor",
                "a strange, grim satisfaction in the difficulty itself — the awareness that if it were easy, it wouldn't require this, and this is what you came for",
                "the mind partitioning: one section executing the task, another monitoring resources, a third standing watch over the resolve itself — a small, efficient government of will",
                "the thought arriving not as inspiration but as fact: I will finish this — the verb tense already past, the outcome already decided, only the labor remaining",
            ]),
            ("Suppression Cues", vec![
                "keeping the effort invisible — the smooth face, the even breathing, the casual posture concealing the war of attrition happening underneath",
                "deflecting concern with 'I'm fine' spoken in a tone that does not invite follow-up questions",
                "converting visible strain into humor: 'just another Tuesday' or 'this is the fun part' — the joke arriving through gritted teeth",
                "refusing to acknowledge the difficulty to others, reserving all complaints for the internal monologue where they can be noted and dismissed",
                "the controlled movements of someone rationing their remaining energy — nothing wasted, nothing dramatic, every gesture serving the task",
                "maintaining the same pace and tone regardless of increasing difficulty, the consistency itself a form of suppression — no one watching would know the cost is climbing",
                "taking breaks that look voluntary rather than necessary, stepping away with the posture of choice rather than collapse",
                "the single crack in the composure: a longer-than-normal exhale, a moment with the eyes closed, a hand pressed briefly to the face — then back to work",
                "asking for help with logistics but never with the load itself — 'pass me that' but never 'I can't do this'",
                "the private moment of doubt permitted in the space between one effort and the next — acknowledged, felt, and then set aside like a tool that isn't needed for this part",
            ]),
        ]),
        ("Curiosity", "The pulling, energizing desire to know — a pleasant tension between ignorance and discovery", vec![
            ("Physical Signals", vec![
                "the head tilting to one side, the ear leading, the neck offering the brain's listening apparatus to the source — an ancient mammalian signal of interest",
                "eyebrows lifting and holding, the forehead creasing, the face opening its shutters to let in more information",
                "leaning forward from the waist, the torso closing the distance before the feet decide to follow, the body's approach system outpacing its caution",
                "eyes widening, pupils dilating, the visual system literally opening the aperture to take in more light, more detail, more",
                "fingers reaching out to touch — the edge of the object, the texture of the surface, the page of the book — the hands conducting their own investigation",
                "the mouth falling slightly open, the jaw relaxing, the face going soft with attention rather than tight with judgment",
                "turning the object over, holding it up to the light, bringing it closer to the eyes — the methodical examination of someone who needs to understand from every angle",
                "pulling a book off a shelf and opening it standing up, reading the first page without sitting down, without putting down the bag, without taking off the coat",
                "the stillness of someone who has stopped mid-sentence because something more interesting just happened in the periphery",
                "following the sound — the head turning first, then the shoulders, then the feet, the whole body reorienting toward the source like a compass needle finding north",
                "the rapid back-and-forth between the object of interest and a companion's face — 'are you seeing this?' — the instinct to share the discovery",
                "crouching down, kneeling, getting on the same level as the thing being examined — the body lowering itself to meet the mystery where it lives",
                "the absent-minded lip bite or tongue pressing against the inside of the cheek, the mouth working on the problem the brain hasn't solved yet",
                "picking something up that wasn't meant to be picked up — a document on a desk, a tool in a workshop, a stone on a path — the hands disobeying the social contract because the mind needs data",
            ]),
            ("Internal Sensations", vec![
                "a pleasant itch in the prefrontal cortex — not pain but a cognitive deprivation, the specific discomfort of a gap in knowledge demanding to be filled",
                "dopamine releasing in the caudate nucleus and striatum, the reward system treating the promise of information exactly like the promise of food or water",
                "a quickening of the pulse, subtle but measurable, the cardiovascular system responding to novelty with the same mild acceleration it gives to any valued stimulus",
                "a lightness in the chest, the opposite of dread's weight — the ribcage lifting, the breathing shallowing with attention rather than anxiety",
                "the skin tingling at the point of contact — fingertips, palms, the back of the hand — the tactile system amplifying its sensitivity when exploring something new",
                "a narrowing of the sensory field, background noise dimming, peripheral vision softening, the brain reallocating bandwidth to the channel that carries the interesting signal",
                "the specific restlessness of not knowing — a low-grade tension that isn't unpleasant so much as insistent, a thread being pulled that won't let go",
                "a warm, expanding feeling in the chest when the first piece of the answer clicks into place — the micro-reward of partial understanding",
                "the eyes drying slightly from forgetting to blink, the body deprioritizing maintenance in favor of intake",
                "hunger and thirst going unnoticed, the body's basic drives temporarily outranked by the drive to know — the nervous system treating information as a primary need",
            ]),
            ("Mental Responses", vec![
                "the information gap opening like a door ajar — the mind cannot leave it, cannot look away, the incompleteness pulling at attention like gravity",
                "questions proliferating: but why, but how, but what happens if — each answer spawning two more questions, the investigation branching outward",
                "the mental model building in real time, pieces being tried and discarded, hypotheses forming and dissolving, the architecture of understanding under active construction",
                "a fierce, almost physical resistance to being interrupted — the thought 'not now' arriving with startling force when someone breaks the concentration",
                "pattern recognition firing on all cylinders, the mind scanning for connections, analogies, precedents — 'this is like that, which means...'",
                "the delicious uncertainty of not knowing yet — the awareness that the answer exists and is reachable and hasn't been reached, the best part of the chase",
                "time vanishing — looking up to find that an hour has passed in what felt like ten minutes, the clock having been dismissed by the prefrontal cortex as irrelevant",
                "the urge to tell someone what you've found so far, the discovery feeling incomplete until it's been shared, explained, tested against another mind",
                "the mental bookmark being placed on everything else — other tasks, other obligations, other thoughts — all tagged as 'later' so the interesting thing can have the full stage",
                "the quiet thrill of realizing you're wrong about something, the model breaking apart not with the pain of failure but with the excitement of revision — being wrong means there's more to find",
            ]),
            ("Suppression Cues", vec![
                "forcing the eyes away from the interesting thing and back to the task at hand, the gaze dragged back like a dog on a leash",
                "closing the book, the browser tab, the drawer — physically removing the stimulus because the willpower to ignore it isn't sufficient on its own",
                "nodding along to a conversation while the mind is three rooms away, still turning the question over, still worrying the gap",
                "writing it down — the question, the half-formed theory, the thing to look up later — externalizing the itch so the brain will consent to release it temporarily",
                "the casual 'huh, interesting' delivered in a flat tone that conceals the five follow-up questions being forcibly held behind the teeth",
                "channeling the investigative energy into the sanctioned task, trying to find the same fascination in the spreadsheet that the mystery offered freely",
                "checking the time and calculating whether there's room for a quick detour — just five minutes, just a glance — the negotiation between duty and desire",
                "the phone in the pocket, the search query already half-composed, the thumb itching to type it — the modern suppression of curiosity requiring the restraint to not look it up immediately",
                "filing the question under 'someday' with the private knowledge that someday will be tonight, after everyone else has gone to sleep",
                "the deliberate subject change in conversation — steering away from the fascinating tangent before it swallows the meeting whole, the responsible adult overruling the fascinated child",
            ]),
        ]),
        ("Eagerness", "Enthusiasm with impatience — wanting something and leaning hard toward it", vec![
            ("Physical Signals", vec![
                "bouncing on the balls of the feet, the heels barely touching down, the body converting surplus energy into small vertical oscillations",
                "eyes bright and wide, pupils dilated, the face lit from within by a wattage the person doesn't know they're emitting",
                "hands moving constantly — gesturing, pointing, tapping surfaces, reaching toward things — the fingers unable to hold still when the mind is running ahead",
                "leaning so far forward in the chair that the back legs threaten to lift, the body's center of gravity already aimed at the destination",
                "speaking faster than usual, words overlapping, sentences finishing before the thought is fully formed because the next one is already pushing through",
                "the grin that won't be managed — wide, unguarded, showing teeth, the kind that makes the muscles around the eyes crinkle and stay crinkled",
                "nodding vigorously before the other person has finished speaking, the head already agreeing, already past the question and onto the answer",
                "clapping hands together once, sharply — the sound of a body punctuating its own readiness, a self-generated starting pistol",
                "pulling at a jacket, a bag, a door handle a half-second before the signal to go, the hands reaching for the next step while the rest is still catching up",
                "the full-body pivot toward the thing being discussed — not just the head but the shoulders, the hips, the knees, the whole chassis turning to face the object of desire",
                "drumming fingers on a table, a knee, a steering wheel — the rhythm fast and irregular, the body's metronome set to a tempo the situation hasn't reached yet",
                "standing when everyone else is sitting, or sitting with such coiled-spring tension that standing would actually be the more relaxed posture",
                "checking the time not with dread but with impatience, the glance at the clock a challenge rather than a retreat — 'is it time yet, is it time yet'",
                "the rapid inhale through the nose, the kind that precedes speech, taken three or four times before the person is actually given the floor",
            ]),
            ("Internal Sensations", vec![
                "a buzzing, carbonated feeling in the chest, as if the ribs are containing something effervescent that wants to rise and expand",
                "dopamine flooding the mesolimbic pathway, the reward system firing not at the thing itself but at the diminishing distance between now and the thing",
                "the heart rate elevated and steady, not the spike of surprise but the sustained hum of an engine that's been revved and held at high idle",
                "a restless electricity in the legs and feet, the large muscles twitching with surplus activation, the body producing more energy than the moment requires",
                "the stomach tight but not with nausea — with compression, the core cinching itself as if bracing for a sprint that hasn't been called yet",
                "warmth radiating outward from the chest to the arms and face, the vasodilation of positive arousal, the body flushing with go rather than stop",
                "the breath coming in quick, shallow pulls, the lungs cycling faster not from exertion but from the sympathetic nervous system's anticipatory overdrive",
                "a tingling in the palms and fingertips, the hands' nerve endings sharpening, demanding something to grasp, to do, to build",
                "the jaw loosening, the mouth wanting to talk, to laugh, to make sound — the vocal apparatus primed and impatient",
                "the specific frustration of energy with no outlet — the body running its motor against the brakes, vibrating with the effort of holding still when every system says go",
            ]),
            ("Mental Responses", vec![
                "the mind racing three steps ahead of the conversation, already at the doing while everyone else is still at the deciding",
                "impatience not as irritation but as overflow — the thought 'yes, I know, let's go' arriving not from rudeness but from the sheer pressure of wanting to begin",
                "planning the first move, the first word, the first step before permission has been granted, the strategy forming in the background like a program loading",
                "the inability to focus on anything else — other topics bouncing off the surface of attention like rain off a moving car, nothing sticking that isn't the thing",
                "a childlike certainty that this will be good, the critical faculties temporarily on leave, the inner skeptic drowned out by the inner enthusiast",
                "the mental countdown transforming from a measurement of time into a measurement of endurance — not 'ten more minutes' but 'I have to survive ten more minutes'",
                "rehearsing what to say, what to do, how to begin — the mind running a dress rehearsal at double speed, compressing preparation into whatever time remains",
                "the awareness of one's own eagerness arriving as a secondary emotion — noticing the grin, the bouncing, the fast speech, and not caring enough to rein it in",
                "frustration with anything that adds delay: a slow elevator, a long introduction, a procedural step — each one a tollbooth on the road to the thing that matters",
                "the thought looping: 'I can't wait, I can't wait, I can't wait' — not as words but as a rhythm, a pulse, a background frequency the brain is broadcasting without being asked",
            ]),
            ("Suppression Cues", vec![
                "pressing the lips together to physically block the grin, the mouth fighting the face's default setting, the effort visible in the dimples that form anyway",
                "crossing the arms and gripping the biceps, the hands placed under arrest to prevent their gesturing, their reaching, their drumming",
                "the deliberate slow-down of speech, each word placed with care that costs visible effort, the pace of someone driving thirty in a sixty zone",
                "sitting back in the chair with forced casualness, the spine making contact with the backrest for the first time in the conversation",
                "the single deep breath taken through the nose, held, and released — the body's manual override of the sympathetic nervous system's accelerator",
                "responding to 'you seem excited' with a measured 'yeah, it should be good' — the understatement landing like a lid on a pot that's about to boil over",
                "channeling the energy downward, the bouncing knee hidden beneath the table, the tapping foot inaudible on carpet — the eagerness relocated to parts of the body the audience can't see",
                "asking practical questions — logistics, timelines, requirements — converting the raw enthusiasm into an acceptable professional register",
                "the quick tongue-press against the inside of the cheek, the micro-gesture of someone biting back the whoop or the 'yes!' that nearly escaped",
                "arriving early and pretending it was convenient rather than compulsive, standing outside the door with a composure that took the entire walk over to construct",
            ]),
        ]),
        // ── Resignation & Defeat Family ──
        ("Defeat", "The heavy acknowledgment that effort has failed and the outcome cannot be changed", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Resignation", "Defeat accepted — the quiet release of fighting and the hollow settling into what is", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Numbness", "The absence of feeling as a protective response to too much — emotion's circuit breaker", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Boredom", "The restless flatness of under-stimulation — energy with nowhere to go", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        // ── Complex / Mixed States ──
        ("Ambivalence", "Genuinely contradictory feelings held at once toward the same person or situation", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Schadenfreude", "The guilty pleasure in another's misfortune — often immediately followed by shame", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Hiraeth", "Longing for a home, time, or self that may never have fully existed — sorrow with a beautiful edge", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Overwhelm", "The feeling that demands exceed capacity — everything pressing in at once", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Betrayal", "The specific pain of trust violated by someone who was supposed to be safe", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
        ("Alienation", "The sense of being fundamentally separate from others — disconnection from belonging", vec![
            ("Physical Signals", vec![]),
            ("Internal Sensations", vec![]),
            ("Mental Responses", vec![]),
            ("Suppression Cues", vec![]),
        ]),
    ];

    // Non-emotion groups: flat (no sections)
    let flat_groups: Vec<(&str, Vec<&str>)> = vec![
        // ── Sensory ──
        ("Sight — Light & Color", vec![]),
        ("Sight — Shadow & Darkness", vec![]),
        ("Sound — Loud & Harsh", vec![]),
        ("Sound — Quiet & Soft", vec![]),
        ("Sound — Musical & Rhythmic", vec![]),
        ("Smell — Pleasant", vec![]),
        ("Smell — Unpleasant", vec![]),
        ("Taste", vec![]),
        ("Touch — Temperature", vec![]),
        ("Touch — Texture", vec![]),
        ("Touch — Pressure & Pain", vec![]),
        ("Proprioception & Kinesthesia", vec![]),
        // ── Body & Movement ──
        ("Facial Expressions", vec![]),
        ("Eye & Gaze Behavior", vec![]),
        ("Posture & Stance", vec![]),
        ("Gesture & Hand Movement", vec![]),
        ("Gait & Locomotion", vec![]),
        ("Involuntary Physical Responses", vec![]),
        ("Voice & Paralanguage", vec![]),
        // ── Setting & Atmosphere ──
        ("Weather & Sky", vec![]),
        ("Landscape & Exterior", vec![]),
        ("Interior Spaces", vec![]),
        ("Time of Day & Season", vec![]),
        ("Crowd & Social Environment", vec![]),
        ("Symbolic & Atmospheric Objects", vec![]),
        // ── Dialogue & Voice ──
        ("Speech Patterns", vec![]),
        ("Subtext & What Is Unsaid", vec![]),
        ("Argument & Conflict Dialogue", vec![]),
        ("Intimate & Vulnerable Dialogue", vec![]),
        ("Internal Monologue & Thought Voice", vec![]),
    ];

    let mut sort = 0i64;
    for (name, desc, sections) in &groups {
        conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![palette_id, name, desc, sort])?;
        let group_id = conn.last_insert_rowid();
        sort += 1;
        let mut sec_sort = 0i64;
        for (sec_name, words) in sections {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, sec_name, sec_sort])?;
            let section_id = conn.last_insert_rowid();
            sec_sort += 1;
            for (i, word) in words.iter().enumerate() {
                conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, word, i as i64])?;
            }
        }
    }
    for (name, words) in &flat_groups {
        conn.execute("INSERT INTO word_groups (palette_id, name, sort_order) VALUES (?1, ?2, ?3)", params![palette_id, name, sort])?;
        let group_id = conn.last_insert_rowid();
        sort += 1;
        for (i, word) in words.iter().enumerate() {
            conn.execute("INSERT INTO word_entries (group_id, word, sort_order) VALUES (?1, ?2, ?3)", params![group_id, word, i as i64])?;
        }
    }

    Ok(())
}

// ---- Query helpers ----

fn read_palette(row: &rusqlite::Row) -> rusqlite::Result<Palette> {
    Ok(Palette {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        is_system: row.get::<_, i64>(3)? != 0,
        is_active: row.get::<_, i64>(4)? != 0,
        sort_order: row.get(5)?,
        group_count: row.get(6)?,
        entry_count: row.get(7)?,
        created_at: row.get(8)?,
    })
}

const PALETTE_SELECT: &str = "
    SELECT p.id, p.name, p.description, p.is_system, p.is_active, p.sort_order,
           (SELECT COUNT(*) FROM word_groups WHERE palette_id = p.id) as group_count,
           (SELECT COUNT(*) FROM word_entries e JOIN word_groups g ON e.group_id = g.id WHERE g.palette_id = p.id) as entry_count,
           p.created_at
    FROM palettes p";

fn read_group(row: &rusqlite::Row) -> rusqlite::Result<WordGroup> {
    Ok(WordGroup {
        id: row.get(0)?,
        palette_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        sort_order: row.get(4)?,
        entry_count: row.get(5)?,
    })
}

const GROUP_SELECT: &str = "
    SELECT g.id, g.palette_id, g.name, g.description, g.sort_order,
           (SELECT COUNT(*) FROM word_entries WHERE group_id = g.id) as entry_count
    FROM word_groups g";

// ---- Palette Commands ----

#[tauri::command]
pub fn get_palettes(state: tauri::State<'_, PaletteState>) -> Result<Vec<Palette>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!("{} ORDER BY p.sort_order ASC", PALETTE_SELECT)).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| read_palette(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_palette(state: tauri::State<'_, PaletteState>, name: String, description: Option<String>) -> Result<Palette, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM palettes", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO palettes (name, description, sort_order) VALUES (?1, ?2, ?3)", params![name, description, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(&format!("{} WHERE p.id = ?1", PALETTE_SELECT), params![id], |row| read_palette(row)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_palette(state: tauri::State<'_, PaletteState>, id: i64, name: String, description: Option<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE palettes SET name = ?1, description = ?2 WHERE id = ?3 AND is_system = 0", params![name, description, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_palette(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM palettes WHERE id = ?1 AND is_system = 0", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_palette(state: tauri::State<'_, PaletteState>, id: i64, active: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE palettes SET is_active = ?1 WHERE id = ?2", params![active as i64, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn copy_palette(state: tauri::State<'_, PaletteState>, id: i64, new_name: String) -> Result<Palette, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    // Get source palette
    let (desc,): (Option<String>,) = conn.query_row(
        "SELECT description FROM palettes WHERE id = ?1", params![id],
        |row| Ok((row.get(0)?,))
    ).map_err(|e| e.to_string())?;

    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM palettes", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO palettes (name, description, is_system, is_active, sort_order) VALUES (?1, ?2, 0, 1, ?3)", params![new_name, desc, max_sort + 1]).map_err(|e| e.to_string())?;
    let new_palette_id = conn.last_insert_rowid();

    // Copy groups
    let mut group_ids: Vec<(i64, String, Option<String>, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, name, description, sort_order FROM word_groups WHERE palette_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            group_ids.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap(), row.get(3).unwrap()));
        }
    }

    for (old_group_id, gname, gdesc, gsort) in &group_ids {
        conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![new_palette_id, gname, gdesc, gsort]).map_err(|e| e.to_string())?;
        let new_group_id = conn.last_insert_rowid();

        // Copy sections
        let mut old_sections: Vec<(i64, String, i64)> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT id, name, sort_order FROM word_sections WHERE group_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
            let mut rows = stmt.query(params![old_group_id]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                old_sections.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()));
            }
        }

        let mut section_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for (old_sec_id, sname, ssort) in &old_sections {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![new_group_id, sname, ssort]).map_err(|e| e.to_string())?;
            section_map.insert(*old_sec_id, conn.last_insert_rowid());
        }

        // Copy entries
        let mut old_entries: Vec<(Option<i64>, String, i64)> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT section_id, word, sort_order FROM word_entries WHERE group_id = ?1 ORDER BY sort_order").map_err(|e| e.to_string())?;
            let mut rows = stmt.query(params![old_group_id]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                old_entries.push((row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()));
            }
        }

        for (old_sec_id, word, wsort) in &old_entries {
            let new_sec_id = old_sec_id.and_then(|sid| section_map.get(&sid).copied());
            conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![new_group_id, new_sec_id, word, wsort]).map_err(|e| e.to_string())?;
        }
    }

    conn.query_row(&format!("{} WHERE p.id = ?1", PALETTE_SELECT), params![new_palette_id], |row| read_palette(row)).map_err(|e| e.to_string())
}

// ---- Group Commands ----

#[tauri::command]
pub fn get_word_groups(state: tauri::State<'_, PaletteState>, palette_id: i64) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!("{} WHERE g.palette_id = ?1 ORDER BY g.sort_order ASC", GROUP_SELECT)).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![palette_id], |row| read_group(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_word_group(state: tauri::State<'_, PaletteState>, id: i64) -> Result<WordGroupDetail, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = conn.query_row(&format!("{} WHERE g.id = ?1", GROUP_SELECT), params![id], |row| read_group(row)).map_err(|e| e.to_string())?;

    let mut sec_stmt = conn.prepare("SELECT id, group_id, name, sort_order FROM word_sections WHERE group_id = ?1 ORDER BY sort_order ASC").map_err(|e| e.to_string())?;
    let sections = sec_stmt.query_map(params![id], |row| Ok(WordSection { id: row.get(0)?, group_id: row.get(1)?, name: row.get(2)?, sort_order: row.get(3)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let mut entry_stmt = conn.prepare("SELECT id, group_id, section_id, word, sort_order FROM word_entries WHERE group_id = ?1 ORDER BY sort_order ASC").map_err(|e| e.to_string())?;
    let entries = entry_stmt.query_map(params![id], |row| Ok(WordEntry { id: row.get(0)?, group_id: row.get(1)?, section_id: row.get(2)?, word: row.get(3)?, sort_order: row.get(4)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    Ok(WordGroupDetail { group, sections, entries })
}

#[tauri::command]
pub fn create_word_group(state: tauri::State<'_, PaletteState>, palette_id: i64, name: String, description: Option<String>) -> Result<WordGroup, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // Block edits to system palettes
    let is_sys: i64 = conn.query_row("SELECT is_system FROM palettes WHERE id = ?1", params![palette_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if is_sys != 0 { return Err("Cannot modify system palette".to_string()); }

    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_groups WHERE palette_id = ?1", params![palette_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_groups (palette_id, name, description, sort_order) VALUES (?1, ?2, ?3, ?4)", params![palette_id, name, description, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(&format!("{} WHERE g.id = ?1", GROUP_SELECT), params![id], |row| read_group(row)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_word_group(state: tauri::State<'_, PaletteState>, id: i64, name: String, description: Option<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_groups SET name = ?1, description = ?2 WHERE id = ?3", params![name, description, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_word_group(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_groups WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Section Commands ----

#[tauri::command]
pub fn get_all_section_names(state: tauri::State<'_, PaletteState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT DISTINCT name FROM word_sections ORDER BY name ASC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<String>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_section(state: tauri::State<'_, PaletteState>, group_id: i64, name: String) -> Result<WordSection, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_sections WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, name, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(WordSection { id, group_id, name, sort_order: max_sort + 1 })
}

#[tauri::command]
pub fn add_all_sections(state: tauri::State<'_, PaletteState>, group_id: i64, names: Vec<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut existing_stmt = conn.prepare("SELECT name FROM word_sections WHERE group_id = ?1").map_err(|e| e.to_string())?;
    let existing: std::collections::HashSet<String> = existing_stmt.query_map(params![group_id], |row| row.get(0))
        .map_err(|e| e.to_string())?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_sections WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut sort = max_sort + 1;
    for name in &names {
        if !existing.contains(name) {
            conn.execute("INSERT INTO word_sections (group_id, name, sort_order) VALUES (?1, ?2, ?3)", params![group_id, name, sort]).map_err(|e| e.to_string())?;
            sort += 1;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_section(state: tauri::State<'_, PaletteState>, id: i64, name: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_sections SET name = ?1 WHERE id = ?2", params![name, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_section(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE word_entries SET section_id = NULL WHERE section_id = ?1", params![id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_sections WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Entry Commands ----

#[tauri::command]
pub fn add_word_entry(state: tauri::State<'_, PaletteState>, group_id: i64, section_id: Option<i64>, word: String) -> Result<WordEntry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_entries WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, word, max_sort + 1]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(WordEntry { id, group_id, section_id, word, sort_order: max_sort + 1 })
}

#[tauri::command]
pub fn add_word_entries(state: tauri::State<'_, PaletteState>, group_id: i64, section_id: Option<i64>, words: Vec<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut existing = std::collections::HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT LOWER(word) FROM word_entries WHERE group_id = ?1 AND (?2 IS NULL AND section_id IS NULL OR section_id = ?2)").map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![group_id, section_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
        for row in rows { if let Ok(w) = row { existing.insert(w); } }
    }
    let max_sort: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) FROM word_entries WHERE group_id = ?1", params![group_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut sort = max_sort + 1;
    for word in &words {
        let trimmed = word.trim();
        if !trimmed.is_empty() && !existing.contains(&trimmed.to_lowercase()) {
            conn.execute("INSERT INTO word_entries (group_id, section_id, word, sort_order) VALUES (?1, ?2, ?3, ?4)", params![group_id, section_id, trimmed, sort]).map_err(|e| e.to_string())?;
            sort += 1;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn remove_word_entry(state: tauri::State<'_, PaletteState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM word_entries WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Search (across active palettes) ----

/// Strip common English suffixes to get a rough stem for fuzzy matching.
fn rough_stem(word: &str) -> String {
    let w = word.to_lowercase();
    for suffix in &["ness", "ment", "tion", "sion", "ious", "eous", "ible", "able", "ful",
                     "less", "ling", "ally", "ily", "ous", "ive", "ing", "ied", "ier",
                     "est", "ity", "ant", "ent", "ise", "ize", "ely", "ate",
                     "ly", "ed", "er", "en", "al", "ic", "es", "is", "ty", "ry", "ny"] {
        if w.len() > suffix.len() + 2 {
            if let Some(base) = w.strip_suffix(suffix) {
                return base.to_string();
            }
        }
    }
    // Also try stripping trailing 's' for plurals
    if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
        return w[..w.len()-1].to_string();
    }
    w
}

/// Search groups across active palettes.
/// Priority: 5=exact name, 4=name contains, 3=name stem match, 2=description match, 1=content match.
#[tauri::command]
pub fn search_word_groups(state: tauri::State<'_, PaletteState>, query: String) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let lower_query = query.to_lowercase();
    let pattern = format!("%{}%", lower_query);
    let query_stem = rough_stem(&lower_query);

    // Get all active groups
    let mut all_groups = Vec::new();
    {
        let mut stmt = conn.prepare(&format!(
            "{} JOIN palettes p2 ON g.palette_id = p2.id WHERE p2.is_active = 1",
            GROUP_SELECT
        )).map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            all_groups.push(read_group(row).map_err(|e| e.to_string())?);
        }
    }

    let mut scored: Vec<(WordGroup, u8)> = Vec::new();

    for group in all_groups {
        let name_lower = group.name.to_lowercase();

        // Exact name match
        if name_lower == lower_query {
            scored.push((group, 5));
            continue;
        }

        // Name contains query
        if name_lower.contains(&lower_query) {
            scored.push((group, 4));
            continue;
        }

        // Fuzzy name match: stem of query matches name, or stem of name matches query
        // Require stem length >= 4 to avoid overly broad matches
        let name_stem = rough_stem(&name_lower);
        if query_stem.len() >= 4 && name_lower.contains(&query_stem) {
            scored.push((group, 3));
            continue;
        }
        if name_stem.len() >= 4 && lower_query.contains(&name_stem) {
            scored.push((group, 3));
            continue;
        }

        // Description contains exact query (no stem — too loose)
        let desc_match = group.description.as_ref().map(|d| {
            d.to_lowercase().contains(&lower_query)
        }).unwrap_or(false);

        if desc_match {
            scored.push((group, 2));
            continue;
        }

        // Content match: word entries contain exact query
        let has_content: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM word_entries WHERE group_id = ?1 AND LOWER(word) LIKE ?2)",
            params![group.id, pattern],
            |row| row.get(0),
        ).unwrap_or(false);
        if has_content {
            scored.push((group, 1));
            continue;
        }
    }

    // Sort by score descending, then alphabetically
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));

    Ok(scored.into_iter().map(|(g, _)| g).collect())
}

/// Get all groups from all active palettes (for the editor picker)
#[tauri::command]
pub fn get_active_groups(state: tauri::State<'_, PaletteState>) -> Result<Vec<WordGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!(
        "{} JOIN palettes p2 ON g.palette_id = p2.id WHERE p2.is_active = 1 ORDER BY g.name ASC",
        GROUP_SELECT
    )).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| read_group(row)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct SearchHit {
    pub group_id: i64,
    pub group_name: String,
    pub section_name: Option<String>,
    pub word: String,
    pub entry_id: i64,
}

/// Search word entries across active palettes, returning flat results with context.
/// Only returns entries that directly contain the query — no fuzzy/stem for entries.
#[tauri::command]
pub fn search_palette_entries(state: tauri::State<'_, PaletteState>, query: String) -> Result<Vec<SearchHit>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let lower_query = query.to_lowercase();
    let pattern = format!("%{}%", lower_query);

    let mut hits: Vec<SearchHit> = Vec::new();

    let sql = "
        SELECT e.id, e.group_id, e.word, g.name as group_name, s.name as section_name
        FROM word_entries e
        JOIN word_groups g ON e.group_id = g.id
        JOIN palettes p ON g.palette_id = p.id
        LEFT JOIN word_sections s ON e.section_id = s.id
        WHERE p.is_active = 1 AND LOWER(e.word) LIKE ?1
        ORDER BY g.name, s.sort_order, e.sort_order
        LIMIT 50
    ";

    {
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![pattern]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            hits.push(SearchHit {
                entry_id: row.get(0).unwrap(),
                group_id: row.get(1).unwrap(),
                word: row.get(2).unwrap(),
                group_name: row.get(3).unwrap(),
                section_name: row.get(4).unwrap(),
            });
        }
    }

    Ok(hits)
}
