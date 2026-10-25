//! The tests' configs with each installed file given by its path name
//! (`{"$file": "obj/P_EX100.mdlx"}`) [test-room choices]: `title`, `room`,
//! `talk` (its NPC), the model selections they name and the image viewer's
//! title-screen selection. Names and test-room numbers only; the launcher
//! fills in where each file is (T109, T228).
pub const TEST_CONFIGS: &str = r#"{
  "title": {
    "schema_version": 1,
    "title": {
      "$file": "menu/us/title.2ld"
    },
    "system_sounds": {
      "$file": "se/se000.win32.scd"
    },
    "messages": {
      "$file": "msg/us/sys.bar"
    },
    "font_image": {
      "$file": "msg/us/fontimage.bar"
    },
    "font_info": {
      "$file": "msg/us/fontinfo.bar"
    },
    "music": {
      "$file": "vagstream/Title.win32.scd"
    }
  },
  "room": {
    "schema_version": 1,
    "actor_selection": "auto-actor-selection.json",
    "motion_set": {
      "$file": "obj/P_EX100.mset"
    },
    "battle_mode": true,
    "has_weapon": true,
    "battle": {
      "$file": "00battle.bin"
    },
    "ptya_pointer": 1,
    "ground_combo_ids": [
      5,
      4,
      3
    ],
    "air_combo_ids": [
      9,
      10,
      8
    ],
    "system": {
      "$file": "03system.bin"
    },
    "prty_pointer": 1,
    "objentry": {
      "$file": "00objentry.bin"
    },
    "object_model_name": "W_EX010",
    "weapon_selection": "auto-weapon-selection.json",
    "weapon_motion_set": {
      "$file": "obj/W_EX010.mset"
    },
    "sklt_character_id": 1,
    "hud": {
      "$file": "field2d/us/zz0field.2dd"
    },
    "face": {
      "$file": "obj/P_EX100.a.us"
    },
    "menu": {
      "command": {
        "$file": "field2d/us/zz0command.2dd"
      },
      "font_image": {
        "$file": "msg/us/fontimage.bar"
      },
      "font_info": {
        "$file": "msg/us/fontinfo.bar"
      },
      "messages": {
        "$file": "msg/us/sys.bar"
      }
    },
    "enemy": {
      "model_selection": "auto-shadow-selection.json",
      "model": {
        "$file": "obj/M_EX020.mdlx"
      },
      "motion_set": {
        "$file": "obj/M_EX020.mset"
      },
      "weapon_model": {
        "$file": "obj/W_EX010.mdlx"
      },
      "enmp_id": 34,
      "battle_level": 1,
      "offset": [
        0.0,
        0.0,
        250.0
      ],
      "actor_model": {
        "$file": "obj/P_EX100.mdlx"
      }
    },
    "arena": {
      "half_size": 4500.0,
      "wall_height": 800.0,
      "tile": 300.0
    },
    "spawn": [
      0.0,
      10.0,
      -600.0
    ],
    "sounds": [
      {
        "$file": "se/se000.win32.scd"
      },
      {
        "$file": "obj/W_EX010.a.us",
        "replacement": "se/obj/se500"
      },
      {
        "$file": "obj/M_EX020.a.us",
        "replacement": "se/obj/se201"
      }
    ],
    "effects": {
      "weapon": {
        "$file": "obj/W_EX010.a.us"
      },
      "enemy": {
        "$file": "obj/M_EX020.a.us"
      },
      "atlas": {
        "$file": "effect/texcommon.dpd"
      },
      "system": {
        "$file": "00effect.bar"
      }
    },
    "camp": {
      "$file": "menu/us/camp.2ld"
    },
    "music": {
      "field": {
        "$file": "bgm/music118.win32.scd"
      },
      "battle": {
        "$file": "bgm/music119.win32.scd"
      }
    },
    "journal": {
      "book": {
        "$file": "menu/us/jmemo.2ld"
      },
      "tables": {
        "$file": "menu/us/jiminy.bar"
      },
      "messages": {
        "$file": "msg/us/jm.bar"
      },
      "worlds": {
        "$file": "menu/jm_world.2ld"
      },
      "tops": {
        "jm": {
          "$file": "menu/jm_top_jm.2ld"
        },
        "tt": {
          "$file": "menu/jm_top_tt.2ld"
        },
        "tt2": {
          "$file": "menu/jm_top_tt2.2ld"
        }
      },
      "curtain": {
        "$file": "menu/us/pause.2ld"
      }
    }
  },
  "talk": {
    "schema_version": 1,
    "npc": {
      "model_selection": "auto-axel-selection.json",
      "motion_set": {
        "$file": "obj/B_EX110_RTN.mset"
      },
      "target_model": {
        "$file": "obj/B_EX110.mdlx"
      },
      "offset": [
        0,
        0,
        180
      ],
      "yaw": 3.141592653589793,
      "messages": {
        "$file": "msg/us/hb.bar"
      },
      "script": [
        12785
      ],
      "font_info": {
        "$file": "msg/us/fontinfo.bar"
      },
      "font_image": {
        "$file": "msg/us/fontimage.bar"
      },
      "world_messages": {
        "$file": "msg/us/tt.bar"
      },
      "world": 2,
      "camera_shape": {
        "bone": 100,
        "offset": [
          0,
          0,
          0
        ],
        "pos_h": 0,
        "radius": 90,
        "height": 30
      }
    }
  },
  "actor-selection": {
    "schema_version": 1,
    "selections": [
      {
        "$file": "obj/P_EX100.mdlx"
      }
    ],
    "inspect_model_geometry": true,
    "model_pose_profiles": [
      "openkh_legacy_bind_fma_v1"
    ],
    "inspect_model_textures": true,
    "model_texture_modes": [
      "original_indexed"
    ],
    "model_uv_profiles": [
      "openkh_kkdf2_integer_truncated_uv_v1"
    ]
  },
  "weapon-selection": {
    "schema_version": 1,
    "selections": [
      {
        "$file": "obj/W_EX010.mdlx"
      }
    ],
    "inspect_model_geometry": true,
    "model_pose_profiles": [
      "openkh_legacy_bind_fma_v1"
    ],
    "inspect_model_textures": true,
    "model_texture_modes": [
      "original_indexed"
    ],
    "model_uv_profiles": [
      "openkh_kkdf2_integer_truncated_uv_v1"
    ]
  },
  "shadow-selection": {
    "schema_version": 1,
    "selections": [
      {
        "$file": "obj/M_EX020.mdlx"
      }
    ],
    "inspect_model_geometry": true,
    "model_pose_profiles": [
      "openkh_legacy_bind_fma_v1"
    ],
    "inspect_model_textures": true,
    "model_texture_modes": [
      "original_indexed"
    ],
    "model_uv_profiles": [
      "openkh_kkdf2_integer_truncated_uv_v1"
    ]
  },
  "axel-selection": {
    "schema_version": 1,
    "selections": [
      {
        "$file": "obj/B_EX110_RTN.mdlx"
      }
    ],
    "inspect_model_geometry": true,
    "model_pose_profiles": [
      "openkh_legacy_bind_fma_v1"
    ],
    "inspect_model_textures": true,
    "model_texture_modes": [
      "original_indexed"
    ],
    "model_uv_profiles": [
      "openkh_kkdf2_integer_truncated_uv_v1"
    ]
  },
  "title-selection": {
    "schema_version": 1,
    "selections": [
      {
        "$file": "menu/us/title.2ld"
      }
    ],
    "inspect_hd_images": true
  }
}
"#;
