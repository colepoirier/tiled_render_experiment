use bevy::prelude::*;
use csv::Writer;

use crate::types::Tilemap;

// use bevy::ecs::{
//     archetype::Archetypes,
//     component::{ComponentId, Components},
// };

// pub fn get_component_names_for_entity(
//     entity: Entity,
//     archetypes: &Archetypes,
//     components: &Components,
// ) -> Vec<String> {
//     let mut comp_names = vec![];
//     for archetype in archetypes.iter() {
//         if archetype.entities().contains(&entity) {
//             comp_names = archetype.components().collect::<Vec<ComponentId>>();
//         }
//     }
//     comp_names
//         .iter()
//         .map(|c| components.get_info(*c).unwrap().name().to_string())
//         .collect::<Vec<String>>()
// }

// fn list_cameras_system(
//     camera_q: Query<(Entity, &Transform, &OrthographicProjection)>,
//     world: &World,
// ) {
//     for (e, t, proj) in camera_q.iter() {
//         info!(
//             "Camera {e:?}, {t:?}, {proj:?}, with components: {:?}",
//             get_component_names_for_entity(e, &world.archetypes(), &world.components())
//         );
//     }
// }

pub fn get_grid_shape(grid: &Tilemap) -> (u32, u32) {
    let (mut x_min, mut x_max, mut y_min, mut y_max) = (0, 0, 0, 0);
    for &(x, y) in grid.keys() {
        if x < x_min {
            x_min = x;
        } else if x > x_max {
            x_max = x;
        }

        if y < y_min {
            y_min = y;
        } else if y > y_max {
            y_max = y;
        }
    }

    (x_max - x_min + 1, y_max - y_min + 1)
}

pub fn tilemap_stats_and_debug(grid: &Tilemap) {
    let mut counts: Vec<usize> = vec![];

    for v in grid.values() {
        counts.push(v.shapes.len());
    }

    let num_occupied_bins = counts.iter().filter(|x| **x != 0).collect::<Vec<_>>().len();
    let min = counts.iter().min().unwrap();
    let max = counts.iter().max().unwrap();
    let num_rects_incl_duplicates = counts.iter().sum::<usize>();
    // average shapes per occupied bin
    let avg_spob = counts.iter().sum::<usize>() / counts.len();

    let grid_size = get_grid_shape(&grid);

    let mut wtr = Writer::from_path("table_heatmap_data.csv").unwrap();

    for iy in 0..grid_size.1 {
        let mut row = vec![];
        for ix in 0..grid_size.0 {
            let count = grid.get(&(ix, iy)).unwrap().shapes.len();
            row.push(count.to_string());
        }

        wtr.write_record(&row[..]).unwrap();
    }

    wtr.flush().unwrap();

    let num_bins = (grid_size.0 * grid_size.1) as usize;
    let grid_occupancy = num_occupied_bins as f32 / num_bins as f32;
    info!(
        "grid_size: {grid_size:?}, num_bins: {num_bins}, num_occupied_bins: {num_occupied_bins}, num_rects_incl_duplicates: {num_rects_incl_duplicates}"
    );
    info!("grid_occupancy: {grid_occupancy}");
    info!(
        "avg shapes per occupied bin: {}",
        num_rects_incl_duplicates as f32 / num_occupied_bins as f32
    );
    info!("min: {min}, max: {max}, avg_spob: {avg_spob}");
}
