""" Driver for a cli tool to process an eflect data set. """

import os

from argparse import ArgumentParser

from processing import compute_footprint
from protos.sample.sample_pb2 import DataSet

def parse_eflect_args():
    parser = ArgumentParser()
    parser.add_argument(
        'data',
        help='path to an eflect data set'
    )
    parser.add_argument(
        '-o',
        '--output',
        dest='output',
        default=None,
        help='path to write the footprint to'
    )

    args = parser.parse_args()
    if args.output is None:
        os.path.dirname(args.data)

    return args

def load_data_set(data_set_path):
    """ Loads an EflectDataSet from the path. """
    with open(data_set_path, 'rb') as f:
        data_set = DataSet()
        data_set.ParseFromString(f.read())
        return data_set

def write_data(output_dir, footprint):
    """ Write the footprint. """
    footprint.to_csv(os.path.join(output_dir, 'eflect-footprint.csv'))

def main():
    args = parse_eflect_args()

    data = load_data_set(args.data)
    footprint = compute_footprint(data)
    write_data(args.output, footprint)

if __name__ == '__main__':
    main()
