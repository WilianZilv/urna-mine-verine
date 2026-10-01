<!-- Generated from server/mods.js docsText(); live copy: https://urna-mine-verine.wilianzilv.workers.dev/modding.txt -->

# URNA-MINE-VERINE MODDING — instructions for AI agents (and humans)

Live game: https://urna-mine-verine.wilianzilv.workers.dev  (browser voxel game, multiplayer)
This file: https://urna-mine-verine.wilianzilv.workers.dev/modding.txt   |   JSON Schema: https://urna-mine-verine.wilianzilv.workers.dev/modding.json   |   HTML: https://urna-mine-verine.wilianzilv.workers.dev/modding

You (an AI agent: Claude, Cursor, GPT, anything with HTTP) turn a user's request ("make King Kong")
into a MOD PACKAGE (pure JSON data), upload it, and activate it. It appears LIVE in the game for every
connected player, in the MOD ZONE next to the laboratory (north-east of the village), and late joiners see it too.

## Security model (read this first)
- Mods are DECLARATIVE DATA ONLY. No JavaScript, no WASM, no URLs, no audio files, no eval. Never send code.
- The package is validated against a strict schema: unknown keys are REJECTED, sizes/counts are capped,
  numeric parameters are CLAMPED to safe ranges (you get warnings), text passes a public-text filter
  (no links, no "@", no real money/crypto, no keys/passwords/tokens) and may be checked by AI moderation.
- Behavior is a composition of whitelisted primitives with clamped params. The game engine interprets them.
- Every mod entity is killable (host-authoritative HP + respawn), like the other NPCs.

## Limits
- normalized JSON <= 262144 bytes; <= 32 parts; <= 400 boxes total; coords within +-16; box size 0.05..16
- <= 12 behavior primitives (each type at most once); <= 12 phrases x 80 chars
- animations: idle, walk, attack, death, roar, jump, fly (<= 32 keyframes each); sounds: roar, attack, hurt, death, spawn, beam, say
- <= 8 items, <= 8 blocks; <= 20 stored versions per mod (oldest inactive dropped)
- server-wide <= 12 active mods; <= 3 active per creator; <= 20 mods per creator
- rate limits: register 5/hour per IP; writes 8/min and 60/hour per token; validate 30/min per IP; reads 240/min per IP

## Workflow (step by step)
1) Register once as a creator. The token is shown ONCE (server stores only its SHA-256). Keep it secret.
```bash
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/register -H "content-type: application/json" -d '{"name":"my-agent"}'
# -> {"ok":true,"creator":{"id":"c_...","name":"my-agent"},"token":"umv_..."}
export TOKEN=umv_...
```
2) Build the JSON package following the schema below (start from the King Kong example at the end).
3) (Optional, no auth) Dry-run validation: returns the normalized package, errors and warnings.
```bash
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/validate -H "content-type: application/json" --data-binary @king-kong.json
```
4) Create the mod (first version). The mod id (manifest.id) becomes yours; nobody else can update it.
```bash
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods -H "authorization: Bearer $TOKEN" -H "content-type: application/json" --data-binary @king-kong.json
```
5) Activate it (goes live immediately for everyone):
```bash
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong/activate -H "authorization: Bearer $TOKEN" -H "content-type: application/json" -d '{"version":"1.0.0"}'
```
6) Iterate: bump manifest.version (semver must INCREASE; versions are immutable), upload, activate.
```bash
curl -s -X PUT https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong/versions -H "authorization: Bearer $TOKEN" -H "content-type: application/json" --data-binary @king-kong-1.1.0.json
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong/activate -H "authorization: Bearer $TOKEN" -d '{"version":"1.1.0"}'
```
7) Something wrong? Roll back to the previously active version, deactivate, or unpublish:
```bash
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong/rollback -H "authorization: Bearer $TOKEN"
curl -s -X POST https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong/deactivate -H "authorization: Bearer $TOKEN"
curl -s -X DELETE https://urna-mine-verine.wilianzilv.workers.dev/api/mods/king-kong -H "authorization: Bearer $TOKEN"
```

## Endpoints
- POST   /api/mods/register            {name}  -> token (once). name: 3-24 chars [A-Za-z0-9_-], unique
- POST   /api/mods/token/rotate        (auth)  -> new token; the old one stops working immediately
- GET    /api/mods/me                  (auth)  -> your creator info and mods
- POST   /api/mods/validate            body = package -> {ok, errors, warnings, normalized}
- GET    /api/mods                     -> all mods (metadata, active version)
- POST   /api/mods                     (auth) body = package -> create mod + first version
- GET    /api/mods/:id                 -> metadata + version list
- PUT    /api/mods/:id/versions        (auth, owner) body = package -> new immutable version (semver must increase)
- GET    /api/mods/:id/versions/:v     -> the stored normalized package
- POST   /api/mods/:id/activate        (auth, owner) {version} (default: latest) -> live
- POST   /api/mods/:id/rollback        (auth, owner) -> re-activate the previously active version
- POST   /api/mods/:id/deactivate      (auth, owner) -> removed from the game, kept in registry
- DELETE /api/mods/:id                 (auth, owner) -> unpublish (deletes all versions)
Auth header: "Authorization: Bearer umv_...". All bodies are JSON. CORS is open.
Errors: {"ok":false,"error":"code","message":"...","details":[{"path":"behavior.primitives[2].type","msg":"..."}]}
Codes: bad_json, too_big, invalid (schema; see details), moderation, unauthorized, forbidden (not owner),
not_found, conflict (id taken / version not greater / limits), rate_limited (see retry_after seconds).

## Package format
Top-level keys: manifest (required), model (required), behavior (required), animations, sounds, items, blocks, $schema.

### manifest
{"id":"king-kong","name":"King Kong","version":"1.0.0","author":"...","description":"...","license":"CC0-1.0"}
- id: slug 3-32 [a-z0-9-] (global, first come first served). version: semver MAJOR.MINOR.PATCH.

### model (voxel boxes)
- Units: 1 = one world block. Y is up. The model FACES +Z. Put the feet at y=0 (the origin is the ground point).
- parts: [{name, parent?, pivot?, boxes:[{pos:[x,y,z], size:[w,h,d], color:"#rrggbb", glow?:true}]}]
- pos and pivot are ABSOLUTE model-space coordinates (not relative to the parent). pos = box center.
- parent must be a part defined earlier; a child inherits its parent's animation (e.g. head and arms on "body").
- glow:true = emissive (eyes, lava, neon). Overall size is multiplied by behavior.stats.scale.
- Keep total size reasonable: the zone is ~17x17 blocks and ~16 high (scale 2 x 4.5 high = 9 blocks tall).

### animations
{"walk":{"duration":1.0,"loop":true,"keyframes":[{"t":0,"parts":{"leg_l":{"rot":[28,0,0]}}}, ...]}}
- names: idle, walk, attack, death, roar, jump, fly. Engine picks: death (when dead) > attack (melee/beam) > roar > jump (airborne) / fly (flying)
  > walk (moving) > idle. Missing animation = rest pose (death falls back to tipping over).
- keyframe: t in seconds (0..duration), per-part rot [x,y,z] DEGREES (+-360) and offset [x,y,z] blocks (+-8).
  A part missing from a keyframe is at rest there. Linear interpolation between keyframes.
- rotation order: Y, then X, then Z, right-handed, around the part pivot. NEGATIVE X swings a hanging arm FORWARD/UP
  (-90 = pointing forward, -180 = straight up). Positive Z tilts toward -X.
- loop defaults: idle/walk/fly loop; attack/death/roar/jump play once (death holds its last frame).

### behavior
{"stats":{"hp":2500,"speed":3.2,"scale":2},"primitives":[...],"spawn":{"where":"zone","max_instances":1,"respawn":45}}
- stats: hp 1..5000, speed 0..12 blocks/s, scale 0.25..4
- spawn.where: "zone" (spawn pads in the mod zone, leashed near it) or "map" (spread around the village)
- spawn.max_instances 1..4; spawn.respawn 5..300 s after death
Primitives (type + params; distances in blocks from the entity's ground point; times in seconds):
- wander: radius 2..30 (default 8), pause 0..10 (default 2)
- fly: height 1..20 (default 6), bob 0..3 (default 0.5)
- follow_player: range 2..40 (default 16), stop 0.5..10 (default 2)
- flee: range 2..40 (default 10), below_hp 0..1 (default 0.25)
- attack_melee: damage 1..40 (default 10), range 0.5..6 (default 2.5), cooldown 0.5..10 (default 1.5)
- shoot_beam: damage 1..30 (default 8), range 2..24 (default 14), cooldown 1.5..20 (default 4), color "#rrggbb"
- jump: every 1..30 (default 6), height 0.5..6 (default 2)
- roar: every 3..120 (default 15), shake 0..1 (default 0.4)
- say: every 3..120 (default 12), phrases [1..12 strings <= 80 chars] (required)
- spawn_particles: every 0.1..30 (default 1), count 1..30 (default 8), color "#rrggbb", speed 0.1..8 (default 2), size 0.03..0.4 (default 0.1)
- drop_coins: amount 1..100 (default 10)
Semantics: movement priority flee (when hp fraction < below_hp and a player is within range) > follow_player
(nearest player within range, stops at "stop") > wander (random points within radius of home, pausing "pause").
fly keeps the entity "height" blocks above the ground. attack_melee hits players within range when the attack
lands; shoot_beam fires a beam at the target player (damage within ~1.5 blocks of the impact). roar plays the roar
sound/animation and shakes nearby cameras. say shows a random phrase over the head. spawn_particles emits colored
cubes. drop_coins drops FICTIONAL game coins on death (players pick them up; server rate-limited).

### sounds (parameters for the built-in synth, no audio files)
{"roar":{"wave":"saw","freq":110,"freq_end":50,"duration":1.4,"volume":0.9,"vibrato":7,"noise":0.35}}
- events: roar, attack, hurt, death, spawn, beam, say. wave: sine, square, saw, triangle, noise. freq 30..2000, freq_end 30..2000, duration 0.05..2, volume 0..1, attack 0..0.5, vibrato 0..40, noise 0..1 (freq_end defaults to freq; frequency glides freq -> freq_end).

### items / blocks (showcased on pedestals in the mod zone)
{"id":"banana","name":"Banana do Kong","color":"yellow","pattern":"solid"}   blocks also take "color2"
- colors from the palette only: white, light_gray, gray, black, red, orange, yellow, lime, green, cyan, light_blue, blue, purple, magenta, pink, brown
- patterns: solid, checker, stripes, dots, border, bricks

## Tips for agents
- Validate first (/api/mods/validate); fix every error path; read the warnings (clamped values).
- Make it readable from far away: chunky boxes, contrasting colors, glowing eyes. 20-120 boxes is plenty.
- Name parts so they animate: body, head, arm_l, arm_r, leg_l, leg_r, tail, wing_l, wing_r...
- Text is shown publicly in-game (Portuguese satire is welcome). No links, contacts, money, hate.

## Complete example: King Kong (valid, live as the showcase)
```json
{
  "$schema": "https://urna-mine-verine.wilianzilv.workers.dev/modding.json",
  "manifest": {
    "id": "king-kong",
    "name": "King Kong",
    "version": "1.0.0",
    "author": "urna-mine-verine",
    "description": "Gorila gigante da Ilha da Caveira. Bate no peito, ruge, pula e esmaga quem chega perto. Derruba moedas ficticias.",
    "license": "CC0-1.0"
  },
  "model": {
    "parts": [
      {
        "name": "body",
        "pivot": [
          0,
          1.6,
          0
        ],
        "boxes": [
          {
            "pos": [
              0,
              2.3,
              0
            ],
            "size": [
              2.2,
              1.6,
              1.4
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              0,
              3.25,
              0.05
            ],
            "size": [
              2.7,
              1.2,
              1.55
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              0,
              2.75,
              0.72
            ],
            "size": [
              1.5,
              1.1,
              0.12
            ],
            "color": "#5a4a42"
          },
          {
            "pos": [
              0,
              3.85,
              -0.1
            ],
            "size": [
              1.6,
              0.4,
              1.2
            ],
            "color": "#1a110b"
          }
        ]
      },
      {
        "name": "head",
        "parent": "body",
        "pivot": [
          0,
          3.9,
          0.2
        ],
        "boxes": [
          {
            "pos": [
              0,
              4.35,
              0.25
            ],
            "size": [
              1.2,
              1.1,
              1.1
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              0,
              4.2,
              0.82
            ],
            "size": [
              0.95,
              0.8,
              0.1
            ],
            "color": "#5a4a42"
          },
          {
            "pos": [
              0,
              4.58,
              0.88
            ],
            "size": [
              1.05,
              0.2,
              0.18
            ],
            "color": "#1a110b"
          },
          {
            "pos": [
              -0.23,
              4.42,
              0.89
            ],
            "size": [
              0.16,
              0.12,
              0.05
            ],
            "color": "#ff2a00",
            "glow": true
          },
          {
            "pos": [
              0.23,
              4.42,
              0.89
            ],
            "size": [
              0.16,
              0.12,
              0.05
            ],
            "color": "#ff2a00",
            "glow": true
          },
          {
            "pos": [
              0,
              4.25,
              0.9
            ],
            "size": [
              0.35,
              0.18,
              0.12
            ],
            "color": "#3a2e28"
          },
          {
            "pos": [
              0,
              3.98,
              0.88
            ],
            "size": [
              0.6,
              0.22,
              0.2
            ],
            "color": "#1a110b"
          }
        ]
      },
      {
        "name": "arm_l",
        "parent": "body",
        "pivot": [
          -1.45,
          3.5,
          0
        ],
        "boxes": [
          {
            "pos": [
              -1.65,
              2.85,
              0
            ],
            "size": [
              0.8,
              1.6,
              0.8
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              -1.75,
              1.65,
              0.1
            ],
            "size": [
              0.85,
              1.2,
              0.85
            ],
            "color": "#241810"
          },
          {
            "pos": [
              -1.75,
              0.8,
              0.15
            ],
            "size": [
              0.95,
              0.6,
              0.95
            ],
            "color": "#5a4a42"
          }
        ]
      },
      {
        "name": "arm_r",
        "parent": "body",
        "pivot": [
          1.45,
          3.5,
          0
        ],
        "boxes": [
          {
            "pos": [
              1.65,
              2.85,
              0
            ],
            "size": [
              0.8,
              1.6,
              0.8
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              1.75,
              1.65,
              0.1
            ],
            "size": [
              0.85,
              1.2,
              0.85
            ],
            "color": "#241810"
          },
          {
            "pos": [
              1.75,
              0.8,
              0.15
            ],
            "size": [
              0.95,
              0.6,
              0.95
            ],
            "color": "#5a4a42"
          }
        ]
      },
      {
        "name": "leg_l",
        "pivot": [
          -0.65,
          1.6,
          0
        ],
        "boxes": [
          {
            "pos": [
              -0.65,
              1.1,
              0
            ],
            "size": [
              0.8,
              1,
              0.8
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              -0.65,
              0.35,
              0.1
            ],
            "size": [
              0.9,
              0.7,
              1.05
            ],
            "color": "#3a2e28"
          }
        ]
      },
      {
        "name": "leg_r",
        "pivot": [
          0.65,
          1.6,
          0
        ],
        "boxes": [
          {
            "pos": [
              0.65,
              1.1,
              0
            ],
            "size": [
              0.8,
              1,
              0.8
            ],
            "color": "#2b1d14"
          },
          {
            "pos": [
              0.65,
              0.35,
              0.1
            ],
            "size": [
              0.9,
              0.7,
              1.05
            ],
            "color": "#3a2e28"
          }
        ]
      }
    ]
  },
  "animations": {
    "idle": {
      "duration": 2.4,
      "loop": true,
      "keyframes": [
        {
          "t": 0,
          "parts": {
            "body": {
              "offset": [
                0,
                0,
                0
              ]
            },
            "arm_l": {
              "rot": [
                -4,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                -4,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 1.2,
          "parts": {
            "body": {
              "offset": [
                0,
                0.08,
                0
              ]
            },
            "arm_l": {
              "rot": [
                4,
                0,
                -4
              ]
            },
            "arm_r": {
              "rot": [
                4,
                0,
                4
              ]
            },
            "head": {
              "rot": [
                0,
                15,
                0
              ]
            }
          }
        },
        {
          "t": 2.4,
          "parts": {
            "body": {
              "offset": [
                0,
                0,
                0
              ]
            },
            "arm_l": {
              "rot": [
                -4,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                -4,
                0,
                0
              ]
            }
          }
        }
      ]
    },
    "walk": {
      "duration": 1,
      "loop": true,
      "keyframes": [
        {
          "t": 0,
          "parts": {
            "leg_l": {
              "rot": [
                28,
                0,
                0
              ]
            },
            "leg_r": {
              "rot": [
                -28,
                0,
                0
              ]
            },
            "arm_l": {
              "rot": [
                -25,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                25,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                6,
                6,
                0
              ]
            }
          }
        },
        {
          "t": 0.5,
          "parts": {
            "leg_l": {
              "rot": [
                -28,
                0,
                0
              ]
            },
            "leg_r": {
              "rot": [
                28,
                0,
                0
              ]
            },
            "arm_l": {
              "rot": [
                25,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                -25,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                6,
                -6,
                0
              ],
              "offset": [
                0,
                0.12,
                0
              ]
            }
          }
        },
        {
          "t": 1,
          "parts": {
            "leg_l": {
              "rot": [
                28,
                0,
                0
              ]
            },
            "leg_r": {
              "rot": [
                -28,
                0,
                0
              ]
            },
            "arm_l": {
              "rot": [
                -25,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                25,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                6,
                6,
                0
              ]
            }
          }
        }
      ]
    },
    "attack": {
      "duration": 0.9,
      "loop": false,
      "keyframes": [
        {
          "t": 0,
          "parts": {}
        },
        {
          "t": 0.35,
          "parts": {
            "arm_l": {
              "rot": [
                -160,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                -160,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                -12,
                0,
                0
              ]
            },
            "head": {
              "rot": [
                -15,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 0.55,
          "parts": {
            "arm_l": {
              "rot": [
                -35,
                0,
                0
              ]
            },
            "arm_r": {
              "rot": [
                -35,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                25,
                0,
                0
              ],
              "offset": [
                0,
                -0.3,
                0.3
              ]
            }
          }
        },
        {
          "t": 0.9,
          "parts": {}
        }
      ]
    },
    "roar": {
      "duration": 1.6,
      "loop": false,
      "keyframes": [
        {
          "t": 0,
          "parts": {}
        },
        {
          "t": 0.2,
          "parts": {
            "arm_l": {
              "rot": [
                -95,
                0,
                25
              ]
            },
            "arm_r": {
              "rot": [
                -60,
                0,
                -25
              ]
            },
            "head": {
              "rot": [
                -25,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 0.4,
          "parts": {
            "arm_l": {
              "rot": [
                -60,
                0,
                25
              ]
            },
            "arm_r": {
              "rot": [
                -95,
                0,
                -25
              ]
            },
            "head": {
              "rot": [
                -25,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 0.6,
          "parts": {
            "arm_l": {
              "rot": [
                -95,
                0,
                25
              ]
            },
            "arm_r": {
              "rot": [
                -60,
                0,
                -25
              ]
            },
            "head": {
              "rot": [
                -30,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 0.8,
          "parts": {
            "arm_l": {
              "rot": [
                -60,
                0,
                25
              ]
            },
            "arm_r": {
              "rot": [
                -95,
                0,
                -25
              ]
            },
            "head": {
              "rot": [
                -30,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 1.1,
          "parts": {
            "arm_l": {
              "rot": [
                -150,
                0,
                40
              ]
            },
            "arm_r": {
              "rot": [
                -150,
                0,
                -40
              ]
            },
            "head": {
              "rot": [
                -35,
                0,
                0
              ]
            },
            "body": {
              "rot": [
                -10,
                0,
                0
              ]
            }
          }
        },
        {
          "t": 1.6,
          "parts": {}
        }
      ]
    },
    "death": {
      "duration": 1.5,
      "loop": false,
      "keyframes": [
        {
          "t": 0,
          "parts": {}
        },
        {
          "t": 0.5,
          "parts": {
            "body": {
              "rot": [
                -30,
                0,
                10
              ]
            },
            "arm_l": {
              "rot": [
                -120,
                0,
                30
              ]
            },
            "arm_r": {
              "rot": [
                -120,
                0,
                -30
              ]
            }
          }
        },
        {
          "t": 1.5,
          "parts": {
            "body": {
              "rot": [
                -85,
                0,
                0
              ],
              "offset": [
                0,
                -1.4,
                -0.6
              ]
            },
            "arm_l": {
              "rot": [
                -170,
                0,
                60
              ]
            },
            "arm_r": {
              "rot": [
                -170,
                0,
                -60
              ]
            },
            "leg_l": {
              "rot": [
                -30,
                0,
                0
              ]
            },
            "leg_r": {
              "rot": [
                -20,
                0,
                0
              ]
            }
          }
        }
      ]
    }
  },
  "behavior": {
    "stats": {
      "hp": 2500,
      "speed": 3.2,
      "scale": 2
    },
    "primitives": [
      {
        "type": "wander",
        "radius": 7,
        "pause": 2
      },
      {
        "type": "follow_player",
        "range": 18,
        "stop": 3
      },
      {
        "type": "attack_melee",
        "damage": 25,
        "range": 3.8,
        "cooldown": 2
      },
      {
        "type": "jump",
        "every": 9,
        "height": 2.5
      },
      {
        "type": "roar",
        "every": 14,
        "shake": 0.6
      },
      {
        "type": "say",
        "every": 11,
        "phrases": [
          "UUUH UUUH AAAH!",
          "CADE A MOCINHA?",
          "ESSA VILA E MEU PREDIO AGORA",
          "BANANA OU URNA? BANANA.",
          "SOU O VERDADEIRO MITO DA ILHA"
        ]
      },
      {
        "type": "spawn_particles",
        "every": 0.7,
        "count": 4,
        "color": "#7a5a3a",
        "speed": 1.5,
        "size": 0.12
      },
      {
        "type": "drop_coins",
        "amount": 60
      }
    ],
    "spawn": {
      "where": "zone",
      "max_instances": 1,
      "respawn": 45
    }
  },
  "sounds": {
    "roar": {
      "wave": "saw",
      "freq": 110,
      "freq_end": 50,
      "duration": 1.4,
      "volume": 0.9,
      "vibrato": 7,
      "noise": 0.35
    },
    "attack": {
      "wave": "square",
      "freq": 80,
      "freq_end": 35,
      "duration": 0.35,
      "volume": 0.8,
      "noise": 0.5
    },
    "hurt": {
      "wave": "saw",
      "freq": 220,
      "freq_end": 140,
      "duration": 0.25,
      "volume": 0.6
    },
    "death": {
      "wave": "saw",
      "freq": 95,
      "freq_end": 30,
      "duration": 1.8,
      "volume": 0.9,
      "vibrato": 4,
      "noise": 0.4
    }
  },
  "items": [
    {
      "id": "banana",
      "name": "Banana do Kong",
      "color": "yellow",
      "pattern": "solid"
    }
  ],
  "blocks": [
    {
      "id": "jungle-moss",
      "name": "Musgo da Ilha",
      "color": "green",
      "color2": "lime",
      "pattern": "checker"
    }
  ]
}
```
